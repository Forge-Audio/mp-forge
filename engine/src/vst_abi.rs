//! The shapes a plugin and its host exchange, as Akai's own header gives them (`framework/mpc_vst2_abi.h`).
//!
//! Only what the live player uses is here. Every field is in the header's order, because the plugin
//! reads and writes these by position.

use std::ffi::c_void;

pub const EFFECT_MAGIC: i32 = 0x5673_7450;
pub const FLAG_IS_INSTRUMENT: i32 = 1 << 8;
/// Names the plugin may be exported under, most usual first.
pub const ENTRY_NAMES: [&[u8]; 3] = [b"VSTPluginMain\0", b"main_macho\0", b"main\0"];

pub type HostCallback = extern "C" fn(effect: *mut AEffect, opcode: i32, index: i32, value: isize, pointer: *mut c_void, opt: f32) -> isize;
pub type PluginEntry = unsafe extern "C" fn(host: HostCallback) -> *mut AEffect;
pub type Dispatcher = unsafe extern "C" fn(effect: *mut AEffect, opcode: i32, index: i32, value: isize, pointer: *mut c_void, opt: f32) -> isize;
pub type ProcessBlock = unsafe extern "C" fn(effect: *mut AEffect, inputs: *mut *mut f32, outputs: *mut *mut f32, frames: i32);
pub type ProcessDoubleBlock = unsafe extern "C" fn(effect: *mut AEffect, inputs: *mut *mut f64, outputs: *mut *mut f64, frames: i32);
pub type SetParameter = unsafe extern "C" fn(effect: *mut AEffect, index: i32, amount: f32);
pub type GetParameter = unsafe extern "C" fn(effect: *mut AEffect, index: i32) -> f32;

#[repr(C)]
pub struct AEffect {
    pub magic: i32,
    pub dispatcher: Option<Dispatcher>,
    pub process: Option<ProcessBlock>,
    pub set_parameter: Option<SetParameter>,
    pub get_parameter: Option<GetParameter>,
    pub num_programs: i32,
    pub num_params: i32,
    pub num_inputs: i32,
    pub num_outputs: i32,
    pub flags: i32,
    pub reserved_1: isize,
    pub reserved_2: isize,
    pub initial_delay: i32,
    pub real_qualities: i32,
    pub off_qualities: i32,
    pub io_ratio: f32,
    pub object: *mut c_void,
    pub user: *mut c_void,
    pub unique_id: i32,
    pub version: i32,
    pub process_replacing: Option<ProcessBlock>,
    pub process_double_replacing: Option<ProcessDoubleBlock>,
    pub future: [u8; 56],
}

/// What the host asks of the plugin.
pub mod to_plugin {
    pub const OPEN: i32 = 0;
    pub const CLOSE: i32 = 1;
    pub const SET_PROGRAM: i32 = 2;
    pub const GET_PROGRAM: i32 = 3;
    pub const GET_PARAM_LABEL: i32 = 6;
    pub const GET_PARAM_DISPLAY: i32 = 7;
    pub const SET_SAMPLE_RATE: i32 = 10;
    pub const SET_BLOCK_SIZE: i32 = 11;
    pub const MAINS_CHANGED: i32 = 12;
    pub const PROCESS_EVENTS: i32 = 25;
    pub const GET_PROGRAM_NAME_INDEXED: i32 = 29;
    pub const START_PROCESS: i32 = 71;
    pub const STOP_PROCESS: i32 = 72;
}

/// What the plugin asks of the host.
pub mod to_host {
    /// The plugin says it has moved one of its own controls.
    pub const AUTOMATE: i32 = 0;
    pub const VERSION: i32 = 1;
    pub const GET_TIME: i32 = 7;
    pub const PROCESS_EVENTS: i32 = 8;
    pub const IO_CHANGED: i32 = 13;
    pub const GET_SAMPLE_RATE: i32 = 16;
    pub const GET_BLOCK_SIZE: i32 = 17;
    pub const GET_VENDOR_STRING: i32 = 32;
    pub const GET_PRODUCT_STRING: i32 = 33;
    pub const GET_VENDOR_VERSION: i32 = 34;
    pub const CAN_DO: i32 = 37;
    pub const UPDATE_DISPLAY: i32 = 42;
}

pub const HOST_VST_VERSION: isize = 2400;
pub const MIDI_EVENT: i32 = 1;
pub const MIDI_EVENT_IS_REALTIME: i32 = 1;
pub const MOST_EVENTS_IN_A_BLOCK: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VstMidiEvent {
    pub kind: i32,
    pub byte_size: i32,
    pub delta_frames: i32,
    pub flags: i32,
    pub note_length: i32,
    pub note_offset: i32,
    pub midi_data: [u8; 4],
    pub detune: i8,
    pub note_off_velocity: i8,
    pub reserved_1: i8,
    pub reserved_2: i8,
}

/// The header declares room for two events and says the list is as long as `num_events` in practice.
#[repr(C)]
pub struct VstEvents {
    pub num_events: i32,
    pub reserved: isize,
    pub events: [*mut VstMidiEvent; MOST_EVENTS_IN_A_BLOCK],
}

pub mod time_flags {
    pub const TRANSPORT_PLAYING: i32 = 1 << 1;
    pub const NANOS_VALID: i32 = 1 << 8;
    pub const PPQ_POS_VALID: i32 = 1 << 9;
    pub const TEMPO_VALID: i32 = 1 << 10;
    pub const BARS_VALID: i32 = 1 << 11;
    pub const TIME_SIG_VALID: i32 = 1 << 13;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VstTimeInfo {
    pub sample_pos: f64,
    pub sample_rate: f64,
    pub nano_seconds: f64,
    pub ppq_pos: f64,
    pub tempo: f64,
    pub bar_start_pos: f64,
    pub cycle_start_pos: f64,
    pub cycle_end_pos: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub smpte_offset: i32,
    pub smpte_frame_rate: i32,
    pub samples_to_next_clock: i32,
    pub flags: i32,
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use std::mem::size_of;

    use super::*;

    /// The sizes a C++ compiler gives these on a 64-bit computer. A wrong size means a field is out of place.
    #[test]
    fn the_shapes_are_the_size_the_plugin_expects() {
        assert_eq!(size_of::<AEffect>(), 192);
        assert_eq!(size_of::<VstMidiEvent>(), 32);
        assert_eq!(size_of::<VstTimeInfo>(), 88);
        assert_eq!(size_of::<VstEvents>(), 16 + 8 * MOST_EVENTS_IN_A_BLOCK);
    }
}
