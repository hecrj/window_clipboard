use crate::error::Error;
use std::thread;
use std::time::{Duration, Instant};
use xcb::base::ConnError;
use xcb::{Atom, Connection, Window};

const POLL_DURATION: std::time::Duration = Duration::from_micros(50);

#[derive(Clone, Debug)]
pub struct Atoms {
    // pub primary: Atom,
    pub clipboard: Atom,
    pub property: Atom,
    // pub targets: Atom,
    // pub string: Atom,
    pub utf8_string: Atom,
    pub incr: Atom,
}

/// X11 Clipboard
pub struct Clipboard {
    connection: Connection,
    window: Window,
    pub atoms: Atoms,
}

#[inline]
fn get_atom(connection: &Connection, name: &str) -> Result<Atom, Error> {
    xcb::intern_atom(connection, false, name)
        .get_reply()
        .map(|reply| reply.atom())
        .map_err(Into::into)
}

impl Clipboard {
    /// Create Clipboard.
    pub fn new(displayname: Option<&str>) -> Result<Self, Error> {
        let (connection, screen) = Connection::connect(displayname)?;
        let window = connection.generate_id();

        {
            let screen = connection
                .get_setup()
                .roots()
                .nth(screen as usize)
                .ok_or(Error::XcbConn(ConnError::ClosedInvalidScreen))?;
            xcb::create_window(
                &connection,
                xcb::COPY_FROM_PARENT as u8,
                window,
                screen.root(),
                0,
                0,
                1,
                1,
                0,
                xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
                screen.root_visual(),
                &[(
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_STRUCTURE_NOTIFY
                        | xcb::EVENT_MASK_PROPERTY_CHANGE,
                )],
            );
            connection.flush();
        }

        macro_rules! intern_atom {
            ( $name:expr ) => {
                get_atom(&connection, $name)?
            };
        }

        let atoms = Atoms {
            // primary: xcb::ATOM_PRIMARY,
            clipboard: intern_atom!("CLIPBOARD"),
            property: intern_atom!("THIS_CLIPBOARD_OUT"),
            // targets: intern_atom!("TARGETS"),
            // string: xcb::ATOM_STRING,
            utf8_string: intern_atom!("UTF8_STRING"),
            incr: intern_atom!("INCR"),
        };

        Ok(Clipboard {
            connection,
            window,
            atoms,
        })
    }

    fn process_event<T>(
        &self,
        buff: &mut Vec<u8>,
        selection: Atom,
        target: Atom,
        property: Atom,
        timeout: T,
    ) -> Result<(), Error>
    where
        T: Into<Option<Duration>>,
    {
        let mut is_incr = false;
        let end_time = if let Some(dur) = timeout.into() {
            Some(Instant::now() + dur)
        } else {
            None
        };

        loop {
            if let Some(end) = end_time {
                if Instant::now() > end {
                    return Err(Error::Timeout);
                }
            }

            let event = match self.connection.poll_for_event() {
                Some(event) => event,
                None => {
                    thread::park_timeout(POLL_DURATION);
                    continue;
                }
            };

            let r = event.response_type();

            match r & !0x80 {
                xcb::SELECTION_NOTIFY => {
                    let event = unsafe {
                        xcb::cast_event::<xcb::SelectionNotifyEvent>(&event)
                    };
                    if event.selection() != selection {
                        continue;
                    };

                    // Note that setting the property argument to None indicates that the
                    // conversion requested could not be made.
                    if event.property() == xcb::ATOM_NONE {
                        break;
                    }

                    let reply = xcb::get_property(
                        &self.connection,
                        false,
                        self.window,
                        event.property(),
                        xcb::ATOM_ANY,
                        buff.len() as u32,
                        ::std::u32::MAX, // FIXME reasonable buffer size
                    )
                    .get_reply()?;

                    if reply.type_() == self.atoms.incr {
                        if let Some(&size) = reply.value::<i32>().get(0) {
                            buff.reserve(size as usize);
                        }
                        xcb::delete_property(
                            &self.connection,
                            self.window,
                            property,
                        );
                        self.connection.flush();
                        is_incr = true;
                        continue;
                    } else if reply.type_() != target {
                        return Err(Error::UnexpectedType(reply.type_()));
                    }

                    buff.extend_from_slice(reply.value());
                    break;
                }
                xcb::PROPERTY_NOTIFY if is_incr => {
                    let event = unsafe {
                        xcb::cast_event::<xcb::PropertyNotifyEvent>(&event)
                    };
                    if event.state() != xcb::PROPERTY_NEW_VALUE as u8 {
                        continue;
                    };

                    let length = xcb::get_property(
                        &self.connection,
                        false,
                        self.window,
                        property,
                        xcb::ATOM_ANY,
                        0,
                        0,
                    )
                    .get_reply()
                    .map(|reply| reply.bytes_after())?;

                    let reply = xcb::get_property(
                        &self.connection,
                        true,
                        self.window,
                        property,
                        xcb::ATOM_ANY,
                        0,
                        length,
                    )
                    .get_reply()?;

                    if reply.type_() != target {
                        continue;
                    };

                    if reply.value_len() != 0 {
                        buff.extend_from_slice(reply.value());
                    } else {
                        break;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    /// load value.
    pub fn load<T>(
        &self,
        selection: Atom,
        target: Atom,
        property: Atom,
        timeout: T,
    ) -> Result<Vec<u8>, Error>
    where
        T: Into<Option<Duration>>,
    {
        let mut buff = Vec::new();
        let timeout = timeout.into();

        xcb::convert_selection(
            &self.connection,
            self.window,
            selection,
            target,
            property,
            xcb::CURRENT_TIME, // FIXME ^
                               // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                               // Instead, they should use the timestamp of the event that caused the request to be made.
        );
        self.connection.flush();

        self.process_event(&mut buff, selection, target, property, timeout)?;
        xcb::delete_property(&self.connection, self.window, property);
        self.connection.flush();
        Ok(buff)
    }
}
