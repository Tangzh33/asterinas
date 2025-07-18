// SPDX-License-Identifier: MPL-2.0

//! The i8042 mouse driver.

use ostd::{
    trap::TrapFrame,
    sync::Mutex,
};
use alloc::sync::Arc;

use aster_input::{InputDevice, InputDeviceMeta, InputEvent, input_event, InputID};

use aster_time::tsc::read_instant;
use alloc::vec;
use alloc::vec::Vec;
use crate::alloc::string::ToString;
use super::MOUSE_CALLBACKS;
use aster_input::event_type_codes::*;

use crate::DATA_PORT;

pub fn init() {
    log::error!("This is init in kernel/comps/mouse/src/i8042_mouse.rs");
    aster_input::register_device("PS/2 Generic Mouse".to_string(), Arc::new(I8042Mouse));
}


struct I8042Mouse;

impl InputDevice for I8042Mouse {
    fn metadata(&self) -> InputDeviceMeta {
        let id = InputID {
            bustype: 0x11,
            vendor_id: 0x2,
            product_id: 0x1,  
            version: 0,       
        };
        InputDeviceMeta {
            name: "PS/2 Generic Mouse".to_string(),
            phys: "isa0060/serio1/input0".to_string(),
            uniq: "NULL".to_string(),
            version: 65537,
            id: id,
        }
    }

    fn get_prop_bit(&self) -> Vec<PropType> {
        vec![PropType::Pointer]
    }

    fn get_ev_bit(&self) -> Vec<EventType> {
        vec![EventType::EvSyn, EventType::EvKey, EventType::EvRel]
    }

    fn get_key_bit(&self) -> Vec<KeyEvent> {
        vec![KeyEvent::BtnLeft, KeyEvent::BtnMiddle, KeyEvent::BtnRight]
    }

    fn get_led_bit(&self) -> Vec<LedEvent> {
        vec![]
    }

    fn get_msc_bit(&self) -> Vec<MiscEvent> {
        vec![]
    }

    fn get_rel_bit(&self) -> Vec<RelEvent> {
        vec![RelEvent::RelX, RelEvent::RelY]
    }
}

pub struct MouseState {
    buffer: [u8; 3],
    index: usize,
}

static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState { buffer: [0; 3], index: 0 });

pub fn handle_mouse_input(_trap_frame: &TrapFrame) {
    // log::error!("-----This is handle_mouse_input in kernel/comps/i8042_controller/src/i8042_mouse.rs");
    let byte = MousePacket::read_one_byte();

    let mut state = MOUSE_STATE.lock();

    if state.index == 0 && (byte & 0x08 == 0) {
        log::error!("Invalid first byte! Abort.");
        state.index = 0;
        return;
    }
    let index = state.index;
    state.buffer[index] = byte;
    state.index += 1;

    if state.index == 3 {
        let packet = parse_input_packet(state.buffer);
        state.index = 0;
        handle_mouse_packet(packet);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MousePacket {
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub x_movement: i8,
    pub y_movement: i8,
    pub x_overflow: bool,
    pub y_overflow: bool,
}

impl MousePacket {
    fn read_one_byte() -> u8 {
        DATA_PORT.get().unwrap().read()
    }
}

fn parse_input_packet(packet: [u8; 3]) -> MousePacket {
    // log::error!("This is parse_input_packet in kernel/comps/mouse/src/i8042_mouse.rs packet: {:?}", packet);

    let byte0 = packet[0];
    let byte1 = packet[1];
    let byte2 = packet[2];

    MousePacket {
        left_button:   byte0 & 0x01 != 0,
        right_button:  byte0 & 0x02 != 0,
        middle_button: byte0 & 0x04 != 0,
        x_overflow:    byte0 & 0x40 != 0,
        y_overflow:    byte0 & 0x80 != 0,
        x_movement:    byte1 as i8,
        y_movement:   -(byte2 as i8),
    }
}

static LAST_MOUSE_BUTTONS: Mutex<(bool, bool, bool)> = Mutex::new((false, false, false));

fn parse_input_events(packet: MousePacket, prev_buttons: (bool, bool, bool)) -> (Vec<InputEvent>, (bool, bool, bool)) {
    let mut events = Vec::new();

    // Get the current time in microseconds
    let now = read_instant();
    let time_in_microseconds = now.secs() * 1_000_000 + (now.nanos() / 1_000) as u64;

    // Add X movement event if applicable
    if packet.x_movement != 0 {
        events.push(InputEvent {
            time: time_in_microseconds,
            type_: EventType::EvRel as u16,
            code: RelEvent::RelX as u16,
            value: packet.x_movement as i32,
        });
    }

    // Add Y movement event if applicable
    if packet.y_movement != 0 {
        events.push(InputEvent {
            time: time_in_microseconds,
            type_: EventType::EvRel as u16,
            code: RelEvent::RelY as u16,
            value: packet.y_movement as i32,
        });
    }

    // Add button press/release events
    let (prev_left, prev_right, prev_middle) = prev_buttons;
    let current = (packet.left_button, packet.right_button, packet.middle_button);

    if packet.left_button != prev_left {
        events.push(InputEvent {
            time: time_in_microseconds,
            type_: EventType::EvKey as u16,
            code: MouseKeyEvent::MouseLeft as u16,
            value: if packet.left_button { 1 } else { 0 },
        });
    }

    if packet.right_button != prev_right {
        events.push(InputEvent {
            time: time_in_microseconds,
            type_: EventType::EvKey as u16,
            code: MouseKeyEvent::MouseRight as u16,
            value: if packet.right_button { 1 } else { 0 },
        });
    }

    if packet.middle_button != prev_middle {
        events.push(InputEvent {
            time: time_in_microseconds,
            type_: EventType::EvKey as u16,
            code: MouseKeyEvent::MouseMiddle as u16,
            value: if packet.middle_button { 1 } else { 0 },
        });
    }

    (events, current)
}

fn handle_mouse_packet(packet: MousePacket) {
    let mut last_buttons = LAST_MOUSE_BUTTONS.lock();
    let (mut events, new_buttons) = parse_input_events(packet, *last_buttons); 
    *last_buttons = new_buttons; 
    
    // Add a SYNC event to signal the end of the event group
    events.push(InputEvent {
        time: 0,
        type_: EventType::EvSyn as u16,
        code: 0,
        value: 0,
    });
    // Process each event
    for event in events {
        // println!("----------------Event: {:?}", event);
        input_event(event, "PS/2 Generic Mouse");
    }

    // FIXME: the callbacks are going to be replaced.
    for callback in MOUSE_CALLBACKS.lock().iter() {
        callback();
    }
}