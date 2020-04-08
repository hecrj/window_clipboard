// Implements basic clipboard functionality for X11. This is signficantly more
// complex than might be expected. This article is a great read on the subject:
// https://www.uninformativ.de/blog/postings/2017-04-02/0/POSTING-en.html

use crate::error::Error;
use crate::run::{run, SetMap};
use std::ffi::{c_void, CStr};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use xcb::{Atom, ConnError, Connection, Window};

const POLL_DURATION: std::time::Duration = Duration::from_millis(10);

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
    setter_conn: Arc<Connection>,
    window: Window,
    setter_window: Window,
    setmap: SetMap,
    send: Sender<Atom>,
    pub atoms: Atoms,
}

#[inline]
pub fn get_atom(connection: &Connection, name: &str) -> Result<Atom, Error> {
    xcb::intern_atom(connection, false, name)
        .get_reply()
        .map(|reply| reply.atom())
        .map_err(Into::into)
}

impl Clipboard {
    /// Create Clipboard from an XLib display
    pub unsafe fn new_xlib(display: *mut c_void) -> Result<Self, Error> {
        // Note: we *must* create a new connection since we require an
        // independent event loop, hence we only get the display name here.
        let s = x11::xlib::XDisplayString(display as *mut x11::xlib::Display);
        let displayname = CStr::from_ptr(s).to_str().ok();
        Self::new(displayname)
    }

    /// Create Clipboard from an XCB connection
    pub unsafe fn new_xcb(_connection: *mut c_void) -> Result<Self, Error> {
        // Note: we *must* create a new connection since we require an
        // independent event loop, hence we only get the display name here.
        // TODO: get display name from connection
        Self::new(None)
    }

    /// Create Clipboard from an optional display name
    fn new(displayname: Option<&str>) -> Result<Self, Error> {
        let make_window = || -> Result<(Connection, Window), Error> {
            let (connection, screen) = Connection::connect(displayname)?;
            let window = connection.generate_id();

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
                std::i16::MIN,
                std::i16::MIN,
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

            Ok((connection, window))
        };

        let (connection, window) = make_window()?;
        let (setter_conn, setter_window) = make_window()?;
        let setter_conn = Arc::new(setter_conn);
        let conn2 = setter_conn.clone();

        let atoms = Atoms {
            // primary: xcb::ATOM_PRIMARY,
            clipboard: get_atom(&connection, "CLIPBOARD")?,
            property: get_atom(&connection, "THIS_CLIPBOARD_OUT")?,
            // string: xcb::ATOM_STRING,
            utf8_string: get_atom(&connection, "UTF8_STRING")?,
            incr: get_atom(&connection, "INCR")?,
        };
        let targets = get_atom(&connection, "TARGETS")?;
        let incr = atoms.incr;

        let (sender, receiver) = channel();

        // Units are "four-byte units", hence multiplication by 4:
        let max_length = connection.get_maximum_request_length() as usize * 4;

        let setmap = SetMap::default();
        let setmap2 = setmap.clone();

        thread::spawn(move || {
            run(&conn2, &setmap2, targets, incr, max_length, &receiver)
        });

        Ok(Clipboard {
            connection,
            setter_conn,
            window,
            setter_window,
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

    /// Read a value from a selection
    ///
    /// Parameters:
    ///
    /// - `selection`: e.g. `atoms.clipboard`
    /// - `target`: the content type desired; e.g. `atoms.utf8_string`
    /// - `timeout`:
    pub fn read<T>(
        &self,
        selection: Atom,
        target: Atom,
        timeout: T,
    ) -> Result<Vec<u8>, Error>
    where
        T: Into<Option<Duration>>,
    {
        let property = self.atoms.property;
        let mut buff = Vec::new();
        let timeout = timeout.into();

        // FIXME: Clients should not use CurrentTime for the time argument of a
        // ConvertSelection request. Instead, they should use the timestamp of
        // the event that caused the request to be made.
        let time = xcb::CURRENT_TIME;

        // Request transfer of selection to property
        xcb::convert_selection(
            &self.connection,
            self.window,
            selection,
            target,
            property,
            time,
        );
        self.connection.flush();

        self.process_event(&mut buff, selection, target, property, timeout)?;
        xcb::delete_property(&self.connection, self.window, property);
        self.connection.flush();
        Ok(buff)
    }

    /// Write a value to a selection
    ///
    /// Parameters:
    ///
    /// - `selection`: e.g. `atoms.clipboard`
    /// - `target`: the content type; e.g. `atoms.utf8_string`
    /// - `value`: the value as a byte array
    pub fn write<T: Into<Vec<u8>>>(
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
            &self.setter_conn,
            self.setter_window,
            selection,
            xcb::CURRENT_TIME,
        );

        self.setter_conn.flush();

        if xcb::get_selection_owner(&self.setter_conn, selection)
            .get_reply()
            .map(|reply| reply.owner() == self.setter_window)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(Error::Owner)
        }
    }
}
