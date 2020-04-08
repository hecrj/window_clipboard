use std::cmp;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use xcb::{self, Atom, Connection};

const INCR_CHUNK_SIZE: usize = 4000;

// Map of (Selection, (Target, Value))
pub type SetMap = Arc<RwLock<HashMap<Atom, (Atom, Vec<u8>)>>>;

macro_rules! try_continue {
    ( $expr:expr ) => {
        match $expr {
            Some(val) => val,
            None => continue,
        }
    };
}

struct IncrState {
    selection: Atom,
    requestor: Atom,
    property: Atom,
    pos: usize,
}

pub fn run(
    connection: &Arc<Connection>,
    setmap: &SetMap,
    targets: Atom,
    incr: Atom,
    max_length: usize,
    receiver: &Receiver<Atom>,
) {
    let mut incr_map = HashMap::new();
    let mut state_map = HashMap::new();

    while let Some(event) = connection.wait_for_event() {
        // Abort any on-going INCR sends, since our value just changed:
        while let Ok(selection) = receiver.try_recv() {
            if let Some(property) = incr_map.remove(&selection) {
                state_map.remove(&property);
            }
        }

        match event.response_type() & !0x80 {
            xcb::SELECTION_REQUEST => {
                let event = unsafe {
                    xcb::cast_event::<xcb::SelectionRequestEvent>(&event)
                };
                let read_map = try_continue!(setmap.read().ok());
                let &(target, ref value) =
                    try_continue!(read_map.get(&event.selection()));

                // TODO: support target == MULTIPLE. This is required by ICCCM,
                // but apparently isn't breaking anything?
                if event.target() == targets {
                    // Notify recipient of supported targets. We do not support
                    // multiple targets or conversion, so this is simple.
                    xcb::change_property(
                        &connection,
                        xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(),
                        event.property(),
                        xcb::ATOM_ATOM,
                        32,
                        &[targets, target],
                    );
                } else if value.len() < max_length - 24 {
                    // Directly send the value.
                    xcb::change_property(
                        &connection,
                        xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(),
                        event.property(),
                        target,
                        8,
                        value,
                    );
                } else {
                    // Send via INCR.
                    xcb::change_window_attributes(
                        &connection,
                        event.requestor(),
                        &[(
                            xcb::CW_EVENT_MASK,
                            xcb::EVENT_MASK_PROPERTY_CHANGE,
                        )],
                    );
                    xcb::change_property(
                        &connection,
                        xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(),
                        event.property(),
                        incr,
                        32,
                        &[0u8; 0],
                    );

                    incr_map.insert(event.selection(), event.property());
                    state_map.insert(
                        event.property(),
                        IncrState {
                            selection: event.selection(),
                            requestor: event.requestor(),
                            property: event.property(),
                            pos: 0,
                        },
                    );
                }

                xcb::send_event(
                    &connection,
                    false,
                    event.requestor(),
                    0,
                    &xcb::SelectionNotifyEvent::new(
                        event.time(),
                        event.requestor(),
                        event.selection(),
                        event.target(),
                        event.property(),
                    ),
                );
                connection.flush();
            }
            xcb::PROPERTY_NOTIFY => {
                let event = unsafe {
                    xcb::cast_event::<xcb::PropertyNotifyEvent>(&event)
                };
                if event.state() != xcb::PROPERTY_DELETE as u8 {
                    continue;
                };

                let is_end = {
                    let state = try_continue!(state_map.get_mut(&event.atom()));
                    let read_setmap = try_continue!(setmap.read().ok());
                    let &(target, ref value) =
                        try_continue!(read_setmap.get(&state.selection));

                    let len =
                        cmp::min(INCR_CHUNK_SIZE, value.len() - state.pos);
                    xcb::change_property(
                        &connection,
                        xcb::PROP_MODE_REPLACE as u8,
                        state.requestor,
                        state.property,
                        target,
                        8,
                        &value[state.pos..][..len],
                    );

                    state.pos += len;
                    len == 0
                };

                if is_end {
                    state_map.remove(&event.atom());
                }
                connection.flush();
            }
            xcb::SELECTION_CLEAR => {
                let event = unsafe {
                    xcb::cast_event::<xcb::SelectionClearEvent>(&event)
                };
                if let Some(property) = incr_map.remove(&event.selection()) {
                    state_map.remove(&property);
                }
                if let Ok(mut write_setmap) = setmap.write() {
                    write_setmap.remove(&event.selection());
                }
            }
            _ => (),
        }
    }
}
