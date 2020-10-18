use crate::error::Error;
use std::{cmp, collections::HashMap, sync::{Arc, RwLock, mpsc::{Receiver, Sender, channel}}, thread};
use std::time::{Duration, Instant};
use xcb::base::ConnError;
use xcb::{Atom, Connection, Window};
type SetMap = Arc<RwLock<HashMap<Atom, (Atom, Vec<u8>)>>>;

pub const INCR_CHUNK_SIZE: usize = 4000;
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
    pub setter: Arc<Context>,
    setmap: SetMap,
    send: Sender<Atom>
}

pub struct Context {
    pub connection: Connection,
    pub screen: i32,
    pub window: Window,
    pub atoms: Atoms,
}

#[inline]
fn get_atom(connection: &Connection, name: &str) -> Result<Atom, Error> {
    xcb::intern_atom(connection, false, name)
        .get_reply()
        .map(|reply| reply.atom())
        .map_err(Into::into)
}

impl Context {
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
            primary: xcb::ATOM_PRIMARY,
            clipboard: intern_atom!("CLIPBOARD"),
            property: intern_atom!("THIS_CLIPBOARD_OUT"),
            targets: intern_atom!("TARGETS"),
            string: xcb::ATOM_STRING,
            utf8_string: intern_atom!("UTF8_STRING"),
            incr: intern_atom!("INCR"),
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
        let setter = Arc::new(Context::new(None)?);
        let setter2 = Arc::clone(&setter);
        let setmap = Arc::new(RwLock::new(HashMap::new()));
        let setmap2 = Arc::clone(&setmap);

        let (sender, receiver) = channel();
        let max_length = setter.connection.get_maximum_request_length() as usize * 4;
        thread::spawn(move || run(&setter2, &setmap2, max_length, &receiver));

        Ok(Clipboard { getter, setter, setmap, send: sender })
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

            let event = match self.getter.connection.poll_for_event() {
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
                        &self.getter.connection,
                        false,
                        self.getter.window,
                        event.property(),
                        xcb::ATOM_ANY,
                        buff.len() as u32,
                        ::std::u32::MAX, // FIXME reasonable buffer size
                    )
                    .get_reply()?;

                    if reply.type_() == self.getter.atoms.incr {
                        if let Some(&size) = reply.value::<i32>().get(0) {
                            buff.reserve(size as usize);
                        }
                        xcb::delete_property(
                            &self.getter.connection,
                            self.getter.window,
                            property,
                        );
                        self.getter.connection.flush();
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
                        &self.getter.connection,
                        false,
                        self.getter.window,
                        property,
                        xcb::ATOM_ANY,
                        0,
                        0,
                    )
                    .get_reply()
                    .map(|reply| reply.bytes_after())?;

                    let reply = xcb::get_property(
                        &self.getter.connection,
                        true,
                        self.getter.window,
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
            &self.getter.connection,
            self.getter.window,
            selection,
            target,
            property,
            xcb::CURRENT_TIME, // FIXME ^
                               // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                               // Instead, they should use the timestamp of the event that caused the request to be made.
        );
        self.getter.connection.flush();

        self.process_event(&mut buff, selection, target, property, timeout)?;
        xcb::delete_property(
            &self.getter.connection,
            self.getter.window,
            property,
        );
        self.getter.connection.flush();
        Ok(buff)
    }
    
    /// store value.
    pub fn store<T: Into<Vec<u8>>>(&self, selection: Atom, target: Atom, value: T)
        -> Result<(), Error>
    {
        self.send.send(selection)?;
        self.setmap
            .write()
            .map_err(|_| Error::Lock)?
            .insert(selection, (target, value.into()));

        xcb::set_selection_owner(
            &self.setter.connection,
            self.setter.window, selection,
            xcb::CURRENT_TIME
        );

        self.setter.connection.flush();

        if xcb::get_selection_owner(&self.setter.connection, selection)
            .get_reply()
            .map(|reply| reply.owner() == self.setter.window)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(Error::Owner)
        }
    }
}

macro_rules! try_continue {
    ( $expr:expr ) => {
        match $expr {
            Some(val) => val,
            None => continue
        }
    };
}

struct IncrState {
    selection: Atom,
    requestor: Atom,
    property: Atom,
    pos: usize
}

pub fn run(context: &Arc<Context>, setmap: &SetMap, max_length: usize, receiver: &Receiver<Atom>) {
    let mut incr_map = HashMap::new();
    let mut state_map = HashMap::new();

    while let Some(event) = context.connection.wait_for_event() {
        while let Ok(selection) = receiver.try_recv() {
            if let Some(property) = incr_map.remove(&selection) {
                state_map.remove(&property);
            }
        }

        match event.response_type() & !0x80 {
            xcb::SELECTION_REQUEST => {
                let event = unsafe { xcb::cast_event::<xcb::SelectionRequestEvent>(&event) };
                let read_map = try_continue!(setmap.read().ok());
                let &(target, ref value) = try_continue!(read_map.get(&event.selection()));

                if event.target() == context.atoms.targets {
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(), event.property(), xcb::ATOM_ATOM, 32,
                        &[context.atoms.targets, target]
                    );
                } else if value.len() < max_length - 24 {
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(), event.property(), target, 8,
                        value
                    );
                } else {
                    xcb::change_window_attributes(
                        &context.connection, event.requestor(),
                        &[(xcb::CW_EVENT_MASK, xcb::EVENT_MASK_PROPERTY_CHANGE)]
                    );
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(), event.property(), context.atoms.incr, 32,
                        &[0u8; 0]
                    );

                    incr_map.insert(event.selection(), event.property());
                    state_map.insert(
                        event.property(),
                        IncrState {
                            selection: event.selection(),
                            requestor: event.requestor(),
                            property: event.property(),
                            pos: 0
                        }
                    );
                }

                xcb::send_event(
                    &context.connection, false, event.requestor(), 0,
                    &xcb::SelectionNotifyEvent::new(
                        event.time(),
                        event.requestor(),
                        event.selection(),
                        event.target(),
                        event.property()
                    )
                );
                context.connection.flush();
            },
            xcb::PROPERTY_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::PropertyNotifyEvent>(&event) };
                if event.state() != xcb::PROPERTY_DELETE as u8 { continue };

                let is_end = {
                    let state = try_continue!(state_map.get_mut(&event.atom()));
                    let read_setmap = try_continue!(setmap.read().ok());
                    let &(target, ref value) = try_continue!(read_setmap.get(&state.selection));

                    let len = cmp::min(INCR_CHUNK_SIZE, value.len() - state.pos);
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        state.requestor, state.property, target, 8,
                        &value[state.pos..][..len]
                    );

                    state.pos += len;
                    len == 0
                };

                if is_end {
                    state_map.remove(&event.atom());
                }
                context.connection.flush();
            },
            xcb::SELECTION_CLEAR => {
                let event = unsafe { xcb::cast_event::<xcb::SelectionClearEvent>(&event) };
                if let Some(property) = incr_map.remove(&event.selection()) {
                    state_map.remove(&property);
                }
                if let Ok(mut write_setmap) = setmap.write() {
                    write_setmap.remove(&event.selection());
                }
            },
            _ => ()
        }
    }
}
