#[forbid(unsafe_code)]
mod error;

pub use error::Error;

use x11rb::connection::Connection as _;
use x11rb::errors::ConnectError;
use x11rb::protocol::xproto::{self, Atom, AtomEnum, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection as Connection;
use x11rb::wrapper::ConnectionExt;

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const POLL_DURATION: std::time::Duration = Duration::from_micros(50);

/// A connection to an X11 [`Clipboard`].
pub struct Clipboard {
    reader: Context,
    writer: Arc<Context>,
    selections: Arc<RwLock<HashMap<Atom, (Atom, Vec<u32>)>>>,
    worker: mpsc::Sender<Atom>,
}

impl Clipboard {
    /// Connect to the running X11 server and obtain a [`Clipboard`].
    pub fn connect() -> Result<Self, Error> {
        let reader = Context::new(None)?;
        let writer = Arc::new(Context::new(None)?);
        let selections = Arc::new(RwLock::new(HashMap::new()));
        let (sender, receiver) = mpsc::channel();

        let worker = Worker {
            context: Arc::clone(&writer),
            selections: Arc::clone(&selections),
            receiver,
        };

        thread::spawn(move || worker.run());

        Ok(Clipboard {
            reader,
            writer,
            selections,
            worker: sender,
        })
    }

    /// Read the current [`Clipboard`] value.
    pub fn read(&self) -> Result<String, Error> {
        Ok(String::from_utf8(self.load(
            self.reader.atoms.clipboard,
            self.reader.atoms.utf8_string,
            self.reader.atoms.property,
            std::time::Duration::from_secs(3),
        )?)
        .map_err(Error::InvalidUtf8)?)
    }

    /// Write a new value to the [`Clipboard`].
    pub fn write(&mut self, contents: String) -> Result<(), Error> {
        let selection = self.writer.atoms.clipboard;
        let target = self.writer.atoms.utf8_string;

        let _ = self.worker.send(selection)?;

        self.selections
            .write()
            .map_err(|_| Error::SelectionLocked)?
            .insert(selection, (target, contents.chars().map(u32::from).collect()));

        let _ = xproto::set_selection_owner(
            &self.writer.connection,
            self.writer.window,
            selection,
            x11rb::CURRENT_TIME,
        )?;

        let _ = self.writer.connection.flush()?;

        let reply =
            xproto::get_selection_owner(&self.writer.connection, selection)
                .map_err(Into::into)
                .and_then(|cookie| cookie.reply())?;

        if reply.owner == self.writer.window {
            Ok(())
        } else {
            Err(Error::InvalidOwner)
        }
    }

    /// load value.
    fn load(
        &self,
        selection: Atom,
        target: Atom,
        property: Atom,
        timeout: impl Into<Option<Duration>>,
    ) -> Result<Vec<u8>, Error> {
        let mut buff = Vec::new();
        let timeout = timeout.into();

        let _ = xproto::convert_selection(
            &self.reader.connection,
            self.reader.window,
            selection,
            target,
            property,
            x11rb::CURRENT_TIME, // FIXME ^
                                 // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                                 // Instead, they should use the timestamp of the event that caused the request to be made.
        )?;
        let _ = self.reader.connection.flush()?;

        self.process_event(&mut buff, selection, target, property, timeout)?;

        let _ = xproto::delete_property(
            &self.reader.connection,
            self.reader.window,
            property,
        )?;
        let _ = self.reader.connection.flush()?;

        Ok(buff)
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

            let event = match self.reader.connection.poll_for_event()? {
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
                        &self.reader.connection,
                        false,
                        self.reader.window,
                        event.property,
                        Atom::from(AtomEnum::ANY),
                        buff.len() as u32,
                        ::std::u32::MAX, // FIXME reasonable buffer size
                    )
                    .map_err(Into::into)
                    .and_then(|cookie| cookie.reply())?;

                    if reply.type_ == self.reader.atoms.incr {
                        if let Some(&size) = reply.value.get(0) {
                            buff.reserve(size as usize);
                        }

                        let _ = xproto::delete_property(
                            &self.reader.connection,
                            self.reader.window,
                            property,
                        );

                        let _ = self.reader.connection.flush();
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
                        &self.reader.connection,
                        false,
                        self.reader.window,
                        property,
                        Atom::from(AtomEnum::ANY),
                        0,
                        0,
                    )
                    .map_err(Into::into)
                    .and_then(|cookie| cookie.reply())?
                    .bytes_after;

                    let reply = xproto::get_property(
                        &self.reader.connection,
                        true,
                        self.reader.window,
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
}

pub struct Context {
    pub connection: Connection,
    pub screen: usize,
    pub window: Window,
    pub atoms: Atoms,
}

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

pub struct Worker {
    context: Arc<Context>,
    selections: Arc<RwLock<HashMap<Atom, (Atom, Vec<u32>)>>>,
    receiver: mpsc::Receiver<Atom>,
}

struct IncrState {
    selection: Atom,
    requestor: Atom,
    property: Atom,
    pos: usize,
}

impl Worker {
    pub const INCR_CHUNK_SIZE: usize = 4000;

    pub fn run(self) {
        use x11rb::connection::RequestConnection;

        let mut incr_map = HashMap::new();
        let mut state_map = HashMap::new();

        let max_length = self.context.connection.maximum_request_bytes();

        while let Ok(event) = self.context.connection.wait_for_event() {
            while let Ok(selection) = self.receiver.try_recv() {
                if let Some(property) = incr_map.remove(&selection) {
                    state_map.remove(&property);
                }
            }

            match event {
                Event::SelectionRequest(event) => {
                    let selections = match self.selections.read().ok() {
                        Some(selections) => selections,
                        None => continue,
                    };

                    let &(target, ref value) =
                        match selections.get(&event.selection) {
                            Some(key_value) => key_value,
                            None => continue,
                        };

                    if event.target == self.context.atoms.targets {
                        let data = [self.context.atoms.targets, target];

                        self.context.connection.change_property32(
                            xproto::PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            xproto::AtomEnum::ATOM,
                            &data,
                        )
                        .expect("Change property");
                    } else if value.len() < max_length {
                        let _ = self.context.connection.change_property32(
                            xproto::PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            target,
                            value,
                        )
                        .expect("Change property");
                    } else {
                        let _ = xproto::change_window_attributes(
                            &self.context.connection,
                            event.requestor,
                            &xproto::ChangeWindowAttributesAux::new()
                                .event_mask(xproto::EventMask::PROPERTY_CHANGE),
                        )
                        .expect("Change window attributes");

                        self.context.connection.change_property32(
                            xproto::PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            self.context.atoms.incr,
                            &[],
                        )
                        .expect("Change property");

                        incr_map.insert(event.selection, event.property);
                        state_map.insert(
                            event.property,
                            IncrState {
                                selection: event.selection,
                                requestor: event.requestor,
                                property: event.property,
                                pos: 0,
                            },
                        );
                    }

                    let _ = xproto::send_event(
                        &self.context.connection,
                        false,
                        event.requestor,
                        0u32,
                        xproto::SelectionNotifyEvent {
                            response_type: 31,
                            sequence: event.sequence,
                            time: event.time,
                            requestor: event.requestor,
                            selection: event.selection,
                            target: event.target,
                            property: event.property,
                        },
                    )
                    .expect("Send event");

                    let _ = self.context.connection.flush();
                }
                Event::PropertyNotify(event) => {
                    if event.state != xproto::Property::DELETE {
                        continue;
                    }

                    let is_end = {
                        let state = match state_map.get_mut(&event.atom) {
                            Some(state) => state,
                            None => continue,
                        };

                        let selections = match self.selections.read().ok() {
                            Some(selections) => selections,
                            None => continue,
                        };

                        let &(target, ref value) =
                            match selections.get(&state.selection) {
                                Some(key_value) => key_value,
                                None => continue,
                            };

                        let len = std::cmp::min(
                            Self::INCR_CHUNK_SIZE,
                            value.len() - state.pos,
                        );

                        let _ = self.context.connection.change_property32(
                            xproto::PropMode::REPLACE,
                            state.requestor,
                            state.property,
                            target,
                            &value[state.pos..][..len],
                        )
                        .expect("Change property");

                        state.pos += len;
                        len == 0
                    };

                    if is_end {
                        state_map.remove(&event.atom);
                    }

                    self.context.connection.flush().expect("Flush connection");
                }
                Event::SelectionClear(event) => {
                    if let Some(property) = incr_map.remove(&event.selection) {
                        state_map.remove(&property);
                    }

                    if let Ok(mut write_setmap) = self.selections.write() {
                        write_setmap.remove(&event.selection);
                    }
                }
                _ => (),
            }
        }
    }
}
