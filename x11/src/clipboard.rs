use crate::error::Error;
use crate::run::{run, SetMap};
use std::ffi::c_void;
use std::sync::{
    mpsc::{channel, Sender},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use xcb::{ffi::xcb_connection_t, Atom, Connection, Window};

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
    connection: Arc<Connection>,
    window: Window,
    setmap: SetMap,
    send: Sender<Atom>,
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
    /// Create Clipboard from an XLib display and window
    pub unsafe fn new_xlib(
        display: *mut c_void,
        window: u64,
    ) -> Result<Self, Error> {
        let connection = Connection::new_from_xlib_display(display as *mut _);
        Self::new_(connection, window as u32)
    }

    /// Create Clipboard from an XCB connection and window
    pub unsafe fn new_xcb(
        connection: *mut c_void,
        window: Window,
    ) -> Result<Self, Error> {
        let connection =
            Connection::from_raw_conn(connection as *mut xcb_connection_t);
        Self::new_(connection, window)
    }

    fn new_(connection: Connection, window: Window) -> Result<Self, Error> {
        macro_rules! intern_atom {
            ( $name:expr ) => {
                get_atom(&connection, $name)?
            };
        }

        let atoms = Atoms {
            // primary: xcb::ATOM_PRIMARY,
            clipboard: intern_atom!("CLIPBOARD"),
            property: intern_atom!("THIS_CLIPBOARD_OUT"),
            // string: xcb::ATOM_STRING,
            utf8_string: intern_atom!("UTF8_STRING"),
            incr: intern_atom!("INCR"),
        };
        let targets = intern_atom!("TARGETS");
        let incr = atoms.incr;

        let (sender, receiver) = channel();
        let max_length = connection.get_maximum_request_length() as usize * 4;

        let connection = Arc::new(connection);
        let conn2 = connection.clone();

        let setmap = SetMap::default();
        let setmap2 = setmap.clone();

        thread::spawn(move || {
            run(&conn2, &setmap2, targets, incr, max_length, &receiver)
        });

        Ok(Clipboard {
            connection,
            window,
            setmap,
            send: sender,
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

    /// store value.
    pub fn store<T: Into<Vec<u8>>>(
        &self,
        selection: Atom,
        target: Atom,
        value: T,
    ) -> Result<(), Error> {
        self.send.send(selection)?;
        self.setmap
            .write()
            .map_err(|_| Error::Lock)?
            .insert(selection, (target, value.into()));

        xcb::set_selection_owner(
            &self.connection,
            self.window,
            selection,
            xcb::CURRENT_TIME,
        );

        self.connection.flush();

        if xcb::get_selection_owner(&self.connection, selection)
            .get_reply()
            .map(|reply| reply.owner() == self.window)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(Error::Owner)
        }
    }
}
