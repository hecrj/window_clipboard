use crate::error::Error;

use std::thread;
use std::time::{Duration, Instant};
use x11rb::connection::Connection as _;
use x11rb::errors::ConnectError;
use x11rb::protocol::xproto::{self, Atom, AtomEnum, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection as Connection;

const POLL_DURATION: std::time::Duration = Duration::from_micros(50);

#[derive(Clone, Debug)]
pub struct Atoms {
    pub primary: Atom,
    pub clipboard: Atom,
    pub property: Atom,
    pub targets: Atom,
    pub string: Atom,
    pub utf8_string: Atom,
    pub incr: Atom,
}

/// X11 Clipboard
pub struct Clipboard {
    pub getter: Context,
}

pub struct Context {
    pub connection: Connection,
    pub screen: usize,
    pub window: Window,
    pub atoms: Atoms,
}

#[inline]
fn get_atom(connection: &Connection, name: &str) -> Result<Atom, Error> {
    x11rb::protocol::xproto::intern_atom(connection, false, name.as_bytes())
        .map_err(Into::into)
        .and_then(|cookie| cookie.reply())
        .map(|reply| reply.atom)
        .map_err(Into::into)
}

impl Context {
    pub fn new(displayname: Option<&str>) -> Result<Self, Error> {
        let (connection, screen) = Connection::connect(displayname)?;
        let window = connection.generate_id().map_err(|_| {
            Error::ConnectionFailed(ConnectError::InvalidScreen)
        })?;

        {
            let screen =
                connection.setup().roots.get(screen as usize).ok_or(
                    Error::ConnectionFailed(ConnectError::InvalidScreen),
                )?;

            let _ = xproto::create_window(
                &connection,
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                1,
                1,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &xproto::CreateWindowAux::new().event_mask(
                    xproto::EventMask::STRUCTURE_NOTIFY
                        | xproto::EventMask::PROPERTY_CHANGE,
                ),
            )?;

            let _ = connection.flush()?;
        }

        let atoms = Atoms {
            primary: AtomEnum::PRIMARY.into(),
            clipboard: get_atom(&connection, "CLIPBOARD")?,
            property: get_atom(&connection, "THIS_CLIPBOARD_OUT")?,
            targets: get_atom(&connection, "TARGETS")?,
            string: AtomEnum::STRING.into(),
            utf8_string: get_atom(&connection, "UTF8_STRING")?,
            incr: get_atom(&connection, "INCR")?,
        };

        Ok(Context {
            connection,
            screen,
            window,
            atoms,
        })
    }
}

impl Clipboard {
    /// Create Clipboard.
    pub fn new() -> Result<Self, Error> {
        let getter = Context::new(None)?;

        Ok(Clipboard { getter })
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
        let timeout = timeout.into();
        let start_time = if timeout.is_some() {
            Some(Instant::now())
        } else {
            None
        };

        loop {
            if timeout
                .into_iter()
                .zip(start_time)
                .next()
                .map(|(timeout, time)| (Instant::now() - time) >= timeout)
                .unwrap_or(false)
            {
                return Err(Error::Timeout);
            }

            let event = match self.getter.connection.poll_for_event()? {
                Some(event) => event,
                None => {
                    thread::park_timeout(POLL_DURATION);
                    continue;
                }
            };

            match event {
                Event::SelectionNotify(event) => {
                    if event.selection != selection {
                        continue;
                    };

                    // Note that setting the property argument to None indicates that the
                    // conversion requested could not be made.
                    if event.property == AtomEnum::NONE.into() {
                        break;
                    }

                    let reply = xproto::get_property(
                        &self.getter.connection,
                        false,
                        self.getter.window,
                        event.property,
                        Atom::from(AtomEnum::ANY),
                        buff.len() as u32,
                        ::std::u32::MAX, // FIXME reasonable buffer size
                    )
                    .map_err(Into::into)
                    .and_then(|cookie| cookie.reply())?;

                    if reply.type_ == self.getter.atoms.incr {
                        if let Some(&size) = reply.value.get(0) {
                            buff.reserve(size as usize);
                        }

                        let _ = xproto::delete_property(
                            &self.getter.connection,
                            self.getter.window,
                            property,
                        );

                        let _ = self.getter.connection.flush();
                        is_incr = true;

                        continue;
                    } else if reply.type_ != target {
                        return Err(Error::UnexpectedType(reply.type_));
                    }

                    buff.extend_from_slice(&reply.value);
                    break;
                }
                Event::PropertyNotify(event) if is_incr => {
                    if event.state != xproto::Property::NEW_VALUE {
                        continue;
                    };

                    let length = xproto::get_property(
                        &self.getter.connection,
                        false,
                        self.getter.window,
                        property,
                        Atom::from(AtomEnum::ANY),
                        0,
                        0,
                    )
                    .map_err(Into::into)
                    .and_then(|cookie| cookie.reply())?
                    .bytes_after;

                    let reply = xproto::get_property(
                        &self.getter.connection,
                        true,
                        self.getter.window,
                        property,
                        Atom::from(AtomEnum::ANY),
                        0,
                        length,
                    )
                    .map_err(Into::into)
                    .and_then(|cookie| cookie.reply())?;

                    if reply.type_ != target {
                        continue;
                    };

                    if reply.value_len != 0 {
                        buff.extend_from_slice(&reply.value);
                    } else {
                        break;
                    }
                }
                _ => {}
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

        let _ = xproto::convert_selection(
            &self.getter.connection,
            self.getter.window,
            selection,
            target,
            property,
            x11rb::CURRENT_TIME, // FIXME ^
                                 // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                                 // Instead, they should use the timestamp of the event that caused the request to be made.
        )?;
        let _ = self.getter.connection.flush();

        self.process_event(&mut buff, selection, target, property, timeout)?;

        let _ = xproto::delete_property(
            &self.getter.connection,
            self.getter.window,
            property,
        )?;
        let _ = self.getter.connection.flush()?;

        Ok(buff)
    }
}
