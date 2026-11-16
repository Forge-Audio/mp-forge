//! A built plugin, loaded and run the way the MPC runs one, for the live player.
//!
//! Loading a plugin runs its code inside this process. Only the player does that, and the player is a
//! process of its own, so a plugin that crashes or freezes takes the player down and nothing else.
//! One process hosts one plugin: what the plugin asks its host is answered from this module's own state.

use std::cell::UnsafeCell;
use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

use libloading::Library;

use crate::vst_abi::{
    AEffect, EFFECT_MAGIC, ENTRY_NAMES, FLAG_IS_INSTRUMENT, HOST_VST_VERSION, MIDI_EVENT, MIDI_EVENT_IS_REALTIME, MOST_EVENTS_IN_A_BLOCK, PluginEntry,
    VstEvents, VstMidiEvent, VstTimeInfo, time_flags, to_host, to_plugin,
};

/// The MPC never asks a plugin for more frames than this in one go.
pub const LARGEST_BLOCK: usize = 512;
pub const TEMPO: f64 = 120.0;
const BEATS_IN_A_BAR: f64 = 4.0;
const HOST_NAME: &[u8] = b"MPC 3\0";
const HOST_VERSION: isize = 0x0101;
const TEXT_ROOM: usize = 256;
/// More presets than a musician would step through are not listed.
const MOST_PRESETS_LISTED: i32 = 128;
const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
/// What the plugin may ask whether its host can do, and is told yes.
const HOST_ABILITIES: [&[u8]; 7] =
    [b"sendVstEvents", b"sendVstMidiEvent", b"sendVstTimeInfo", b"receiveVstEvents", b"receiveVstMidiEvent", b"sizeWindow", b"supplyIdle"];

static SAMPLE_RATE: AtomicU32 = AtomicU32::new(44_100);
static FRAMES_PLAYED: AtomicU64 = AtomicU64::new(0);
static TIME_NOW: TimeCell = TimeCell(UnsafeCell::new(None));
/// How often the plugin has told its host that it moved the control being listened for.
static CONTROL_LISTENED_FOR: AtomicI32 = AtomicI32::new(NO_CONTROL);
static TIMES_TOLD_IT_MOVED: AtomicU64 = AtomicU64::new(0);
const NO_CONTROL: i32 = -1;
/// Where the plugin last said it moved each control by itself, kept until `moves_made_by_the_plugin` takes it.
/// An MPC draws a control's pictures again when it is told this way, so the app's pictures follow the same word.
static MOVED_TO: [AtomicU32; MOST_CONTROLS_HEARD_OF] = [const { AtomicU32::new(NOT_MOVED) }; MOST_CONTROLS_HEARD_OF];
const MOST_CONTROLS_HEARD_OF: usize = 512;
/// No amount between 0 and 1 is written this way.
const NOT_MOVED: u32 = u32::MAX;

/// Where the song position is kept for the plugin to read. The plugin is only ever called by one thread
/// at a time, so it only ever asks from one thread at a time.
struct TimeCell(UnsafeCell<Option<VstTimeInfo>>);

unsafe impl Sync for TimeCell {}

/// One note being pressed or let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub pressed: bool,
}

pub struct HostedPlugin {
    effect: *mut AEffect,
    inputs: Vec<Vec<f32>>,
    outputs: Vec<Vec<f32>>,
    /// The notes for the next block and the list that points at them. The plugin may read both until
    /// that block has been rendered, so they are kept here and not on the stack.
    notes: Box<[VstMidiEvent; MOST_EVENTS_IN_A_BLOCK]>,
    note_list: Box<VstEvents>,
    /// Kept so the plugin's code stays loaded for as long as the plugin is in use.
    _library: Library,
}

/// The plugin is handed between threads but only one of them calls into it at a time.
unsafe impl Send for HostedPlugin {}

impl HostedPlugin {
    /// Loads the plugin and takes it through the MPC's start-up steps, ready to play at this sample rate.
    pub fn load(library_file: &Path, sample_rate: u32) -> Result<Self, String> {
        SAMPLE_RATE.store(sample_rate, Ordering::Relaxed);
        FRAMES_PLAYED.store(0, Ordering::Relaxed);
        let library = unsafe { Library::new(library_file) }.map_err(|error| format!("The plugin could not be loaded: {error}"))?;
        let effect = created_by(&library)?;
        let channels = |count: i32| vec![vec![0.0; LARGEST_BLOCK]; usize::try_from(count).unwrap_or(0)];
        let (inputs, outputs) = unsafe { (channels((*effect).num_inputs), channels((*effect).num_outputs)) };
        let notes = Box::new([VstMidiEvent::default(); MOST_EVENTS_IN_A_BLOCK]);
        let note_list = Box::new(VstEvents { num_events: 0, reserved: 0, events: [ptr::null_mut(); MOST_EVENTS_IN_A_BLOCK] });
        let plugin = Self { effect, inputs, outputs, notes, note_list, _library: library };
        plugin.start_up(sample_rate);
        Ok(plugin)
    }

    pub fn control_count(&self) -> usize {
        usize::try_from(unsafe { (*self.effect).num_params }).unwrap_or(0)
    }

    pub fn is_instrument(&self) -> bool {
        unsafe { (*self.effect).flags & FLAG_IS_INSTRUMENT != 0 }
    }

    /// Turns a control to a place between 0 and 1. A control the plugin does not have is left alone.
    pub fn turn_control(&mut self, index: usize, amount: f32) {
        let setter = unsafe { (*self.effect).set_parameter };
        if let (Some(set), Some(index)) = (setter, self.control_number(index)) {
            unsafe { set(self.effect, index, amount.clamp(0.0, 1.0)) };
        }
    }

    /// Where a control stands now, between 0 and 1. A control the plugin does not have stands at 0.
    pub fn control_amount(&self, index: usize) -> f32 {
        let getter = unsafe { (*self.effect).get_parameter };
        match (getter, self.control_number(index)) {
            (Some(get), Some(index)) => unsafe { get(self.effect, index) }.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// From now on, counts how often the plugin tells its host that it moved this control by itself.
    /// An MPC only draws a control's pictures again when it is told.
    pub fn listen_for_moves_of(&self, index: usize) {
        CONTROL_LISTENED_FOR.store(self.control_number(index).unwrap_or(NO_CONTROL), Ordering::Relaxed);
        TIMES_TOLD_IT_MOVED.store(0, Ordering::Relaxed);
    }

    pub fn times_told_it_moved(&self) -> u64 {
        TIMES_TOLD_IT_MOVED.load(Ordering::Relaxed)
    }

    /// The names of the presets built into the plugin, in the order the MPC lists them.
    pub fn preset_names(&self) -> Vec<String> {
        let count = unsafe { (*self.effect).num_programs }.clamp(0, MOST_PRESETS_LISTED);
        (0..count).map(|preset| self.text_from(to_plugin::GET_PROGRAM_NAME_INDEXED, preset).trim().to_string()).collect()
    }

    /// The preset the plugin is on, counted from zero.
    pub fn preset_chosen(&self) -> usize {
        usize::try_from(self.ask(to_plugin::GET_PROGRAM, 0, 0, ptr::null_mut(), 0.0)).unwrap_or(0)
    }

    /// Switches to one of the plugin's presets, which sets its controls. A preset the plugin does not have is ignored.
    pub fn choose_preset(&mut self, preset: usize) {
        if preset < self.preset_names().len() {
            self.ask(to_plugin::SET_PROGRAM, 0, preset as isize, ptr::null_mut(), 0.0);
        }
    }

    /// What the control reads where it is now, with its unit: "540 ms".
    pub fn value_text(&self, index: usize) -> String {
        let Some(index) = self.control_number(index) else { return String::new() };
        let value = self.text_from(to_plugin::GET_PARAM_DISPLAY, index);
        let unit = self.text_from(to_plugin::GET_PARAM_LABEL, index);
        [value.trim(), unit.trim()].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ")
    }

    /// Hands the plugin the notes for the block that is rendered next. Notes beyond what fits are dropped.
    pub fn play_notes(&mut self, notes: &[Note]) {
        let played = notes.len().min(MOST_EVENTS_IN_A_BLOCK);
        if played == 0 {
            return;
        }
        for (event, note) in self.notes.iter_mut().zip(notes) {
            *event = midi_event_of(note);
        }
        self.note_list.num_events = played as i32;
        for (pointer, event) in self.note_list.events.iter_mut().zip(self.notes.iter_mut()).take(played) {
            *pointer = event;
        }
        let note_list: *mut VstEvents = &mut *self.note_list;
        self.ask(to_plugin::PROCESS_EVENTS, 0, 0, note_list.cast(), 0.0);
    }

    /// Runs the plugin over one block of stereo sound. At most `LARGEST_BLOCK` frames are rendered.
    pub fn render(&mut self, played_in: &[[f32; 2]], played_out: &mut [[f32; 2]]) {
        let frames = played_in.len().min(played_out.len()).min(LARGEST_BLOCK);
        self.fill_inputs(&played_in[..frames]);
        self.outputs.iter_mut().for_each(|channel| channel[..frames].fill(0.0));
        note_the_time();
        self.process(frames);
        FRAMES_PLAYED.fetch_add(frames as u64, Ordering::Relaxed);
        self.read_outputs(&mut played_out[..frames]);
    }

    fn start_up(&self, sample_rate: u32) {
        let set_rate_and_block = || {
            self.ask(to_plugin::SET_SAMPLE_RATE, 0, 0, ptr::null_mut(), sample_rate as f32);
            self.ask(to_plugin::SET_BLOCK_SIZE, 0, LARGEST_BLOCK as isize, ptr::null_mut(), 0.0);
        };
        set_rate_and_block();
        self.ask(to_plugin::OPEN, 0, 0, ptr::null_mut(), 0.0);
        self.ask(to_plugin::SET_PROGRAM, 0, 0, ptr::null_mut(), 0.0);
        set_rate_and_block();
        self.ask(to_plugin::MAINS_CHANGED, 0, 1, ptr::null_mut(), 0.0);
        self.ask(to_plugin::START_PROCESS, 0, 0, ptr::null_mut(), 0.0);
    }

    fn control_number(&self, index: usize) -> Option<i32> {
        if index < self.control_count() { i32::try_from(index).ok() } else { None }
    }

    fn ask(&self, opcode: i32, index: i32, value: isize, pointer: *mut c_void, opt: f32) -> isize {
        match unsafe { (*self.effect).dispatcher } {
            Some(dispatch) => unsafe { dispatch(self.effect, opcode, index, value, pointer, opt) },
            None => 0,
        }
    }

    fn text_from(&self, opcode: i32, index: i32) -> String {
        let mut written = [0u8; TEXT_ROOM];
        self.ask(opcode, index, 0, written.as_mut_ptr().cast(), 0.0);
        written[TEXT_ROOM - 1] = 0;
        CStr::from_bytes_until_nul(&written).map(|text| text.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// A plugin with one input hears both sides mixed. One with more than two hears silence on the rest.
    fn fill_inputs(&mut self, played_in: &[[f32; 2]]) {
        let mono = self.inputs.len() == 1;
        for (number, channel) in self.inputs.iter_mut().enumerate() {
            for (sample, frame) in channel.iter_mut().zip(played_in) {
                *sample = match (mono, number) {
                    (true, _) => (frame[0] + frame[1]) / 2.0,
                    (false, side @ (0 | 1)) => frame[side],
                    _ => 0.0,
                };
            }
        }
    }

    fn process(&mut self, frames: usize) {
        let mut input_pointers: Vec<*mut f32> = self.inputs.iter_mut().map(|channel| channel.as_mut_ptr()).collect();
        let mut output_pointers: Vec<*mut f32> = self.outputs.iter_mut().map(|channel| channel.as_mut_ptr()).collect();
        if let Some(process) = unsafe { (*self.effect).process_replacing } {
            unsafe { process(self.effect, input_pointers.as_mut_ptr(), output_pointers.as_mut_ptr(), frames as i32) };
        }
    }

    /// A plugin with one output is heard on both sides. One with none is silent.
    fn read_outputs(&self, played_out: &mut [[f32; 2]]) {
        let left = self.outputs.first();
        let right = self.outputs.get(1).or(left);
        for (number, frame) in played_out.iter_mut().enumerate() {
            *frame = [left.map_or(0.0, |channel| channel[number]), right.map_or(0.0, |channel| channel[number])];
        }
    }
}

impl Drop for HostedPlugin {
    /// The plugin frees itself when it is closed, so it is never touched again after this.
    fn drop(&mut self) {
        self.ask(to_plugin::STOP_PROCESS, 0, 0, ptr::null_mut(), 0.0);
        self.ask(to_plugin::MAINS_CHANGED, 0, 0, ptr::null_mut(), 0.0);
        self.ask(to_plugin::CLOSE, 0, 0, ptr::null_mut(), 0.0);
    }
}

fn created_by(library: &Library) -> Result<*mut AEffect, String> {
    let entry = ENTRY_NAMES.iter().find_map(|name| unsafe { library.get::<PluginEntry>(*name) }.ok());
    let entry = entry.ok_or("The file is not a plugin: it has no way in.")?;
    let effect = unsafe { entry(answer_the_plugin) };
    if effect.is_null() {
        return Err("The plugin refused to start.".to_string());
    }
    let usable = unsafe { (*effect).magic == EFFECT_MAGIC && (*effect).process_replacing.is_some() };
    if usable { Ok(effect) } else { Err("The file is not a plugin the MPC could run.".to_string()) }
}

fn midi_event_of(note: &Note) -> VstMidiEvent {
    let status = if note.pressed { NOTE_ON } else { NOTE_OFF };
    VstMidiEvent {
        kind: MIDI_EVENT,
        byte_size: size_of::<VstMidiEvent>() as i32,
        flags: MIDI_EVENT_IS_REALTIME,
        midi_data: [status, note.pitch.min(127), note.velocity.min(127), 0],
        ..VstMidiEvent::default()
    }
}

/// The song the plugin believes it is in: playing, in 4/4, at a steady tempo.
fn time_after(frames_played: u64, sample_rate: u32) -> VstTimeInfo {
    let beats = frames_played as f64 / f64::from(sample_rate) * TEMPO / 60.0;
    VstTimeInfo {
        sample_pos: frames_played as f64,
        sample_rate: f64::from(sample_rate),
        ppq_pos: beats,
        tempo: TEMPO,
        bar_start_pos: (beats / BEATS_IN_A_BAR).floor() * BEATS_IN_A_BAR,
        time_sig_numerator: 4,
        time_sig_denominator: 4,
        flags: time_flags::TRANSPORT_PLAYING | time_flags::TEMPO_VALID | time_flags::TIME_SIG_VALID | time_flags::PPQ_POS_VALID | time_flags::BARS_VALID,
        ..VstTimeInfo::default()
    }
}

fn note_the_time() -> *mut VstTimeInfo {
    let now = time_after(FRAMES_PLAYED.load(Ordering::Relaxed), SAMPLE_RATE.load(Ordering::Relaxed));
    unsafe {
        let kept = &mut *TIME_NOW.0.get();
        kept.insert(now)
    }
}

/// Answers what the plugin asks of its host, with the answers the MPC gives.
extern "C" fn answer_the_plugin(_effect: *mut AEffect, opcode: i32, index: i32, _value: isize, pointer: *mut c_void, moved_to: f32) -> isize {
    match opcode {
        to_host::AUTOMATE => note_that_it_moved(index, moved_to),
        to_host::VERSION => HOST_VST_VERSION,
        to_host::GET_SAMPLE_RATE => SAMPLE_RATE.load(Ordering::Relaxed) as isize,
        to_host::GET_BLOCK_SIZE => LARGEST_BLOCK as isize,
        to_host::GET_TIME => note_the_time() as isize,
        to_host::GET_VENDOR_STRING | to_host::GET_PRODUCT_STRING => write_host_name(pointer),
        to_host::GET_VENDOR_VERSION => HOST_VERSION,
        to_host::CAN_DO => isize::from(is_a_host_ability(pointer)),
        to_host::UPDATE_DISPLAY | to_host::IO_CHANGED | to_host::PROCESS_EVENTS => 1,
        _ => 0,
    }
}

fn note_that_it_moved(control: i32, amount: f32) -> isize {
    if control == CONTROL_LISTENED_FOR.load(Ordering::Relaxed) {
        TIMES_TOLD_IT_MOVED.fetch_add(1, Ordering::Relaxed);
    }
    let kept_for_it = usize::try_from(control).ok().and_then(|control| MOVED_TO.get(control));
    if let (Some(kept), true) = (kept_for_it, amount.is_finite()) {
        kept.store(amount.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    0
}

/// Every control the plugin has said it moved by itself since this was last asked, with where it moved it to.
/// Nothing here calls into the plugin, so it may be asked from any thread while the plugin plays.
pub fn moves_made_by_the_plugin() -> Vec<(usize, f32)> {
    let taken = |kept: &AtomicU32| Some(kept.swap(NOT_MOVED, Ordering::Relaxed)).filter(|amount| *amount != NOT_MOVED).map(f32::from_bits);
    MOVED_TO.iter().enumerate().filter_map(|(control, kept)| Some((control, taken(kept)?))).collect()
}

fn write_host_name(room: *mut c_void) -> isize {
    if !room.is_null() {
        unsafe { ptr::copy_nonoverlapping(HOST_NAME.as_ptr(), room.cast::<u8>(), HOST_NAME.len()) };
    }
    1
}

fn is_a_host_ability(question: *mut c_void) -> bool {
    if question.is_null() {
        return false;
    }
    let asked = unsafe { CStr::from_ptr(question.cast::<c_char>()) }.to_bytes();
    HOST_ABILITIES.contains(&asked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_song_position_follows_the_frames_played() {
        let two_seconds_in = time_after(88_200, 44_100);

        assert_eq!(two_seconds_in.ppq_pos, 4.0);
        assert_eq!(two_seconds_in.bar_start_pos, 4.0);
        assert_eq!(time_after(66_150, 44_100).bar_start_pos, 0.0);
        assert_ne!(two_seconds_in.flags & time_flags::TRANSPORT_PLAYING, 0);
    }

    #[test]
    fn what_the_plugin_says_it_moved_is_handed_over_once() {
        let tell_the_host = |control: i32, amount: f32| answer_the_plugin(ptr::null_mut(), to_host::AUTOMATE, control, 0, ptr::null_mut(), amount);
        tell_the_host(16, 0.25);
        tell_the_host(16, 0.5);
        tell_the_host(40, 7.0);
        tell_the_host(-1, 0.5);
        tell_the_host(9_000, 0.5);
        tell_the_host(41, f32::NAN);

        assert_eq!(moves_made_by_the_plugin(), [(16, 0.5), (40, 1.0)]);
        assert_eq!(moves_made_by_the_plugin(), []);
    }

    #[test]
    fn a_note_becomes_the_midi_message_the_plugin_reads() {
        let pressed = midi_event_of(&Note { pitch: 60, velocity: 100, pressed: true });
        let let_go = midi_event_of(&Note { pitch: 200, velocity: 0, pressed: false });

        assert_eq!(pressed.midi_data, [0x90, 60, 100, 0]);
        assert_eq!(let_go.midi_data, [0x80, 127, 0, 0]);
        assert_eq!(pressed.byte_size, 32);
    }

    #[test]
    fn a_file_that_is_not_a_plugin_is_refused() {
        let not_a_plugin = std::env::temp_dir().join(format!("workbench-not-a-plugin-{}", std::process::id()));
        std::fs::write(&not_a_plugin, "just words").unwrap();

        let refusal = HostedPlugin::load(&not_a_plugin, 44_100).err().unwrap();

        std::fs::remove_file(&not_a_plugin).unwrap();
        assert!(refusal.starts_with("The plugin could not be loaded"), "{refusal}");
    }
}
