//! The live player: one built plugin, played through the computer's speakers while its controls are turned.
//!
//! It runs as a program of its own, started by the app, so a plugin that crashes or freezes stops the
//! player and nothing else. The app writes one request per line on standard input, as JSON, and reads
//! one answer per line on standard output.
//!
//! An effect is fed a built-in beat, or a sound file of the musician's own when they have chosen one.
//! An instrument is played with notes the app sends. The musician sets how loud it all is, and can pass an
//! effect by to hear what it is fed as it was.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, SupportedStreamConfig};
use serde_json::{Value, json};

use crate::beat_loop::beat_loop;
use crate::hosted_plugin::{HostedPlugin, LARGEST_BLOCK, Note, moves_made_by_the_plugin};
use crate::test_sound;

/// The workbench command that starts the player for one built plugin.
pub const PLAY_COMMAND: &str = "play";

const RATE_WITHOUT_SPEAKERS: u32 = 44_100;
/// Where the volume knob stands until the musician turns it: a little under full.
const USUAL_VOLUME: f32 = 0.9;
/// How long the way out takes to follow a setting from one end to the other.
const GLIDE_SECONDS: f32 = 0.02;
const NOTE_VELOCITY: u8 = 100;
/// About as often as a screen is drawn again.
const LOOKS_FOR_MOVES_EVERY: Duration = Duration::from_millis(33);
const NO_SPEAKERS: &str = "This computer has no sound output to play through.";

/// What the app can ask of the player.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Turn { index: usize, amount: f32 },
    /// Switch to one of the plugin's built-in presets, counted from zero.
    Preset(usize),
    Sound(bool),
    Play(Note),
    /// Feed an effect the sound file at this place, or the built-in beat again when there is none.
    Feed(Option<PathBuf>),
    /// Where the volume knob stands, from 0 for silence to 1 for full.
    Volume(f32),
    /// Hear the effect, or pass it by and hear what it is fed.
    Effect(bool),
}

impl Request {
    /// Reads one line from the app. A line that is not a request the player knows is ignored.
    pub fn from_line(line: &str) -> Option<Self> {
        let asked: Value = serde_json::from_str(line).ok()?;
        if let Some(turn) = asked.get("turn") {
            let index = usize::try_from(turn["index"].as_u64()?).ok()?;
            return Some(Request::Turn { index, amount: turn["amount"].as_f64()? as f32 });
        }
        if let Some(note) = asked.get("note") {
            let pitch = u8::try_from(note["pitch"].as_u64()?).ok()?;
            return Some(Request::Play(Note { pitch, velocity: NOTE_VELOCITY, pressed: note["pressed"].as_bool()? }));
        }
        if let Some(preset) = asked.get("preset") {
            return usize::try_from(preset.as_u64()?).ok().map(Request::Preset);
        }
        if let Some(file) = asked.get("feed") {
            return file.is_null().then_some(Request::Feed(None)).or_else(|| file.as_str().map(|place| Request::Feed(Some(PathBuf::from(place)))));
        }
        if let Some(volume) = asked.get("volume") {
            return volume.as_f64().map(|amount| Request::Volume(amount as f32));
        }
        if let Some(effect) = asked.get("effect") {
            return effect.as_bool().map(Request::Effect);
        }
        asked.get("sound").and_then(Value::as_bool).map(Request::Sound)
    }
}

/// What the musician has set for their own ears. It is kept apart from the rig, so a change never waits for
/// the plugin and the sound never stops for one.
#[derive(Debug)]
pub struct Listening {
    volume: AtomicU32,
    effect_is_heard: AtomicBool,
}

impl Listening {
    fn as_usual() -> Self {
        Self { volume: AtomicU32::new(USUAL_VOLUME.to_bits()), effect_is_heard: AtomicBool::new(true) }
    }

    /// Puts the volume knob somewhere between silence and full, and returns where it then stands.
    pub fn set_volume(&self, amount: f32) -> f32 {
        let within_the_knob = if amount.is_finite() { amount.clamp(0.0, 1.0) } else { USUAL_VOLUME };
        self.volume.store(within_the_knob.to_bits(), Ordering::Relaxed);
        within_the_knob
    }

    /// Hears the effect or passes it by, and returns which.
    pub fn hear_the_effect(&self, heard: bool) -> bool {
        self.effect_is_heard.store(heard, Ordering::Relaxed);
        heard
    }

    /// The knob is squared, so that half way round sounds about half as loud.
    fn level(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed)).powi(2)
    }

    fn effect_is_heard(&self) -> bool {
        self.effect_is_heard.load(Ordering::Relaxed)
    }
}

/// What is to reach the speakers: how loud, and how much of it is the plugin's own. The rest is what it was fed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mix {
    level: f32,
    share_of_the_plugin: f32,
}

/// A setting that moves towards where it is wanted a little with each frame, so a change is never heard as a click.
struct Glide {
    now: f32,
    furthest_in_a_frame: f32,
}

impl Glide {
    fn resting_at(now: f32, sample_rate: u32) -> Self {
        Self { now, furthest_in_a_frame: 1.0 / (GLIDE_SECONDS * sample_rate as f32) }
    }

    fn towards(&mut self, wanted: f32) -> f32 {
        self.now += (wanted - self.now).clamp(-self.furthest_in_a_frame, self.furthest_in_a_frame);
        self.now
    }
}

/// The way out to the speakers. It follows the mix that is wanted, and keeps what it lets through safe for
/// speakers and ears.
struct WayOut {
    level: Glide,
    share_of_the_plugin: Glide,
}

impl WayOut {
    fn resting_at(mix: Mix, sample_rate: u32) -> Self {
        Self { level: Glide::resting_at(mix.level, sample_rate), share_of_the_plugin: Glide::resting_at(mix.share_of_the_plugin, sample_rate) }
    }

    /// Mixes what the plugin made with what it was fed. A block the plugin put anything in that is not a
    /// number counts as silence from the plugin, and nothing louder than full scale gets through.
    fn carry(&mut self, wanted: Mix, sound: Sound, heard: &mut [[f32; 2]]) {
        let all_numbers = sound.made.iter().flatten().all(|sample| sample.is_finite());
        for ((heard, fed), made) in heard.iter_mut().zip(sound.fed).zip(sound.made) {
            let level = self.level.towards(wanted.level);
            let plugin = self.share_of_the_plugin.towards(wanted.share_of_the_plugin);
            for (heard, (fed, made)) in heard.iter_mut().zip(fed.iter().zip(made)) {
                let made = if all_numbers { *made } else { 0.0 };
                *heard = ((made * plugin + fed * (1.0 - plugin)) * level).clamp(-1.0, 1.0);
            }
        }
    }
}

/// One block of sound on its way out: what the plugin was fed, and what it made of it.
#[derive(Clone, Copy)]
struct Sound<'block> {
    fed: &'block [[f32; 2]],
    made: &'block [[f32; 2]],
}

/// The plugin with what it is fed: the beat for an effect, and the notes waiting to be played.
pub struct Rig {
    plugin: HostedPlugin,
    sample_rate: u32,
    beat: Vec<[f32; 2]>,
    place_in_beat: usize,
    waiting_notes: Vec<Note>,
    fed: Vec<[f32; 2]>,
    made: Vec<[f32; 2]>,
    listening: Arc<Listening>,
    way_out: WayOut,
}

impl Rig {
    pub fn around(plugin: HostedPlugin, sample_rate: u32) -> Self {
        let beat = if plugin.is_instrument() { Vec::new() } else { beat_loop(sample_rate) };
        let silence = vec![[0.0; 2]; LARGEST_BLOCK];
        let listening = Arc::new(Listening::as_usual());
        let way_out = WayOut::resting_at(mix_wanted(&listening, plugin.is_instrument()), sample_rate);
        Self { plugin, sample_rate, beat, place_in_beat: 0, waiting_notes: Vec::new(), fed: silence.clone(), made: silence, listening, way_out }
    }

    /// What the musician has set for their own ears, which can be changed while the rig is busy playing.
    pub fn listening(&self) -> Arc<Listening> {
        Arc::clone(&self.listening)
    }

    /// Feeds the plugin another sound from now on, round and round, starting at its beginning.
    fn feed(&mut self, sound: Vec<[f32; 2]>) {
        self.beat = sound;
        self.place_in_beat = 0;
    }

    /// Feeds the plugin silence until its next sound is ready. A sound file takes a while to read, and
    /// without this the sound before it would be heard for that long first.
    pub fn wait_for_its_next_sound(&mut self) {
        self.feed(Vec::new());
    }

    /// Turns a control and returns what it now reads.
    pub fn turn(&mut self, index: usize, amount: f32) -> String {
        self.plugin.turn_control(index, amount);
        self.plugin.value_text(index)
    }

    /// Switches to a preset and returns where it is now, with where every control stands.
    pub fn choose_preset(&mut self, preset: usize) -> Value {
        self.plugin.choose_preset(preset);
        json!({ "chosen": self.plugin.preset_chosen(), "values": self.where_controls_stand() })
    }

    fn where_controls_stand(&self) -> Vec<Value> {
        let standing = |index: usize| json!({ "index": index, "amount": self.plugin.control_amount(index), "text": self.plugin.value_text(index) });
        (0..self.plugin.control_count()).map(standing).collect()
    }

    /// What the app is told once the plugin is loaded: its controls, its kind and its presets.
    fn readiness(&self) -> Value {
        json!({
            "controls": self.plugin.control_count(),
            "instrument": self.plugin.is_instrument(),
            "presets": self.plugin.preset_names(),
            "preset": self.plugin.preset_chosen(),
        })
    }

    pub fn play(&mut self, note: Note) {
        self.waiting_notes.push(note);
    }

    /// Fills `heard` with what the plugin makes next, a block at a time, as loud as the musician has it and
    /// kept safe for speakers and ears. An effect that is passed by goes on working unheard, so its echoes
    /// and tails are where they should be when it is heard again.
    pub fn render(&mut self, heard: &mut [[f32; 2]]) {
        let wanted = mix_wanted(&self.listening, self.plugin.is_instrument());
        for block in heard.chunks_mut(LARGEST_BLOCK) {
            self.feed_the_beat(block.len());
            self.plugin.play_notes(&self.waiting_notes);
            self.waiting_notes.clear();
            self.plugin.render(&self.fed[..block.len()], &mut self.made[..block.len()]);
            self.way_out.carry(wanted, Sound { fed: &self.fed[..block.len()], made: &self.made[..block.len()] }, block);
        }
    }

    fn feed_the_beat(&mut self, frames: usize) {
        for frame in &mut self.fed[..frames] {
            *frame = self.beat.get(self.place_in_beat).copied().unwrap_or_default();
            self.place_in_beat = (self.place_in_beat + 1) % self.beat.len().max(1);
        }
    }
}

/// An instrument is fed nothing, so there is nothing to hear in its place: only an effect can be passed by.
fn mix_wanted(listening: &Listening, is_instrument: bool) -> Mix {
    let plugin_is_heard = is_instrument || listening.effect_is_heard();
    Mix { level: listening.level(), share_of_the_plugin: if plugin_is_heard { 1.0 } else { 0.0 } }
}

/// Runs the player for one plugin until the app hangs up.
pub fn play_on_standard_io(plugin_library: &Path) -> io::Result<()> {
    let speakers = Speakers::of_this_computer();
    let sample_rate = speakers.as_ref().map_or(RATE_WITHOUT_SPEAKERS, Speakers::sample_rate);
    let mut player = match plugin_loaded_from(plugin_library, sample_rate) {
        Ok(plugin) => Player::of(Rig::around(plugin, sample_rate), speakers),
        Err(problem) => return say(&json!({ "problem": problem })),
    };
    say(&json!({ "ready": player.rig_held().readiness() }))?;
    keep_telling_of_its_own_moves();
    for line in io::stdin().lock().lines() {
        if let Some(request) = Request::from_line(&line?) {
            say(&player.answer_to(request))?;
        }
    }
    Ok(())
}

/// One plugin being played: its rig, what the musician has set for their own ears, and the way to the speakers.
struct Player {
    rig: Arc<Mutex<Rig>>,
    listening: Arc<Listening>,
    speakers: Option<Speakers>,
    playing: Option<Stream>,
    /// How many times the effect has been given another sound to work on. Only the latest one is fed to it.
    sounds_asked_for: Arc<AtomicUsize>,
}

fn held(rig: &Mutex<Rig>) -> MutexGuard<'_, Rig> {
    rig.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Player {
    fn of(rig: Rig, speakers: Option<Speakers>) -> Self {
        let listening = rig.listening();
        Self { rig: Arc::new(Mutex::new(rig)), listening, speakers, playing: None, sounds_asked_for: Arc::new(AtomicUsize::new(0)) }
    }

    fn rig_held(&self) -> MutexGuard<'_, Rig> {
        held(&self.rig)
    }

    /// The volume and passing the effect by are set without holding the rig, so the sound goes on unbroken.
    fn answer_to(&mut self, request: Request) -> Value {
        match request {
            Request::Turn { index, amount } => json!({ "value": { "index": index, "amount": amount, "text": self.rig_held().turn(index, amount) } }),
            Request::Preset(preset) => json!({ "preset": self.rig_held().choose_preset(preset) }),
            Request::Play(note) => {
                self.rig_held().play(note);
                json!({ "played": note.pitch })
            }
            Request::Feed(file) => {
                self.feed_from(file);
                json!({ "feeding": true })
            }
            Request::Sound(wanted) => json!({ "sound": self.sounding(wanted) }),
            Request::Volume(amount) => json!({ "volume": self.listening.set_volume(amount) }),
            Request::Effect(heard) => json!({ "effect": self.listening.hear_the_effect(heard) }),
        }
    }

    /// Starts or stops the sound, and answers whether it is now playing.
    fn sounding(&mut self, wanted: bool) -> bool {
        self.playing = if wanted { self.started() } else { Ok(None) }.unwrap_or_else(|problem| {
            let _ = say(&json!({ "problem": problem }));
            None
        });
        self.playing.is_some()
    }

    fn started(&self) -> Result<Option<Stream>, String> {
        let rig = Arc::clone(&self.rig);
        let speakers = self.speakers.as_ref().ok_or_else(|| NO_SPEAKERS.to_string())?;
        speakers.play(move |heard| render_or_stay_silent(&rig, heard)).map(Some)
    }

    /// Changes what an effect is fed. A long file takes a while to read, so that is done apart from the
    /// requests: the effect is fed silence meanwhile, and the player still stops, turns and plays while the
    /// file is read. The app is told `{"fed": true or false}` once it is known.
    fn feed_from(&self, file: Option<PathBuf>) {
        if file.is_some() {
            held(&self.rig).wait_for_its_next_sound();
        }
        let wanted = NewSound {
            file,
            number: self.sounds_asked_for.fetch_add(1, Ordering::SeqCst) + 1,
            sounds_asked_for: Arc::clone(&self.sounds_asked_for),
            rig: Arc::clone(&self.rig),
        };
        thread::spawn(move || wanted.read_and_feed());
    }
}

/// A sound an effect is to be given to work on, on its way to the rig.
struct NewSound {
    /// The sound file, or none for the built-in beat.
    file: Option<PathBuf>,
    number: usize,
    sounds_asked_for: Arc<AtomicUsize>,
    rig: Arc<Mutex<Rig>>,
}

impl NewSound {
    /// A sound that another was asked for after is dropped unsaid, so the last one asked for always wins. A file
    /// that cannot be played is said to be so, and the plugin is left with silence until it is given another.
    fn read_and_feed(self) {
        let sample_rate = held(&self.rig).sample_rate;
        let sound = match &self.file {
            Some(file) => test_sound::read_for_playing(file, sample_rate),
            None => Ok(beat_loop(sample_rate)),
        };
        if self.is_still_wanted() {
            let _ = say(&json!({ "fed": self.fed(sound) }));
        }
    }

    fn is_still_wanted(&self) -> bool {
        self.sounds_asked_for.load(Ordering::SeqCst) == self.number
    }

    fn fed(&self, sound: Result<Vec<[f32; 2]>, String>) -> bool {
        match sound {
            Ok(sound) => {
                held(&self.rig).feed(sound);
                true
            }
            Err(problem) => {
                let _ = say(&json!({ "problem": problem }));
                false
            }
        }
    }
}

/// Passes on what the plugin moves by itself, such as lamps it lights for the notes held, so the app's
/// pictures follow it as an MPC's do. It ends with the player, or when the app has hung up.
fn keep_telling_of_its_own_moves() {
    thread::spawn(|| {
        loop {
            thread::sleep(LOOKS_FOR_MOVES_EVERY);
            let moves = moves_made_by_the_plugin();
            if !moves.is_empty() && say(&told_of(&moves)).is_err() {
                return;
            }
        }
    });
}

fn told_of(moves: &[(usize, f32)]) -> Value {
    let moved: Vec<Value> = moves.iter().map(|(index, amount)| json!({ "index": index, "amount": amount })).collect();
    json!({ "moved": moved })
}

fn say(answer: &Value) -> io::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "{answer}")?;
    out.flush()
}

/// The plugin is loaded from a copy of its own, so a new build never changes the file under a running player.
fn plugin_loaded_from(plugin_library: &Path, sample_rate: u32) -> Result<HostedPlugin, String> {
    let copy = private_copy_of(plugin_library).map_err(|error| format!("The built plugin could not be read: {error}"))?;
    let loaded = HostedPlugin::load(&copy, sample_rate);
    let _ = fs::remove_file(&copy);
    loaded
}

pub(crate) fn private_copy_of(plugin_library: &Path) -> io::Result<PathBuf> {
    let ending = plugin_library.extension().map(|ending| ending.to_string_lossy().into_owned()).unwrap_or_default();
    let copy = std::env::temp_dir().join(format!("workbench-player-{}.{ending}", std::process::id()));
    fs::copy(plugin_library, &copy)?;
    Ok(copy)
}

/// The computer's usual sound output and the format it wants.
pub(crate) struct Speakers {
    device: cpal::Device,
    format: SupportedStreamConfig,
}

impl Speakers {
    pub(crate) fn of_this_computer() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let format = device.default_output_config().ok()?;
        Some(Self { device, format })
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.format.sample_rate()
    }

    /// Plays whatever `render` writes, a stretch of frames at a time, for as long as the stream is kept.
    pub(crate) fn play(&self, render: impl FnMut(&mut [[f32; 2]]) + Send + 'static) -> Result<Stream, String> {
        let stream = match self.format.sample_format() {
            SampleFormat::F32 => self.stream_of::<f32>(render),
            SampleFormat::I16 => self.stream_of::<i16>(render),
            SampleFormat::U16 => self.stream_of::<u16>(render),
            other => Err(format!("This computer's sound output uses a format the player does not speak ({other}).")),
        }?;
        stream.play().map_err(|error| format!("The sound could not be started: {error}"))?;
        Ok(stream)
    }

    fn stream_of<Sample: SizedSample + FromSample<f32>>(&self, mut render: impl FnMut(&mut [[f32; 2]]) + Send + 'static) -> Result<Stream, String> {
        let channels = usize::from(self.format.channels());
        let mut heard: Vec<[f32; 2]> = Vec::new();
        let fill = move |wanted: &mut [Sample], _: &cpal::OutputCallbackInfo| {
            heard.resize(wanted.len() / channels.max(1), [0.0; 2]);
            render(&mut heard);
            write_frames(&heard, wanted, channels);
        };
        let built = self.device.build_output_stream(self.format.config(), fill, |_| {}, None);
        built.map_err(|error| format!("The sound output could not be opened: {error}"))
    }
}

/// While the app is turning a control the rig is busy for a moment, and that block is silent rather than late.
fn render_or_stay_silent(rig: &Mutex<Rig>, heard: &mut [[f32; 2]]) {
    match rig.try_lock() {
        Ok(mut rig) => rig.render(heard),
        Err(_) => heard.fill([0.0; 2]),
    }
}

/// Left and right go to the first two channels. Any others stay silent, and a single channel gets the left.
fn write_frames<Sample: SizedSample + FromSample<f32>>(heard: &[[f32; 2]], wanted: &mut [Sample], channels: usize) {
    for (frame, out) in heard.iter().zip(wanted.chunks_mut(channels.max(1))) {
        for (channel, sample) in out.iter_mut().enumerate() {
            *sample = Sample::from_sample(frame.get(channel).copied().unwrap_or(0.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_of_request_is_read_from_its_line() {
        let turn = Request::from_line(r#"{"turn": {"index": 3, "amount": 0.25}}"#);
        let note = Request::from_line(r#"{"note": {"pitch": 60, "pressed": true}}"#);

        assert_eq!(turn, Some(Request::Turn { index: 3, amount: 0.25 }));
        assert_eq!(note, Some(Request::Play(Note { pitch: 60, velocity: NOTE_VELOCITY, pressed: true })));
        assert_eq!(Request::from_line(r#"{"sound": true}"#), Some(Request::Sound(true)));
        assert_eq!(Request::from_line(r#"{"preset": 2}"#), Some(Request::Preset(2)));
        assert_eq!(Request::from_line(r#"{"preset": -1}"#), None);
    }

    #[test]
    fn an_effect_can_be_fed_a_sound_file_or_the_beat_again() {
        assert_eq!(Request::from_line(r#"{"feed": "/sounds/my loop.wav"}"#), Some(Request::Feed(Some(PathBuf::from("/sounds/my loop.wav")))));
        assert_eq!(Request::from_line(r#"{"feed": null}"#), Some(Request::Feed(None)));
        assert_eq!(Request::from_line(r#"{"feed": 7}"#), None);
    }

    #[test]
    fn a_line_the_player_does_not_know_is_ignored() {
        assert_eq!(Request::from_line("rm -rf /"), None);
        assert_eq!(Request::from_line(r#"{"turn": {"index": -1, "amount": 0.5}}"#), None);
        assert_eq!(Request::from_line(r#"{"note": {"pitch": 900, "pressed": true}}"#), None);
        assert_eq!(Request::from_line(r#"{"shell": "ls"}"#), None);
    }

    #[test]
    fn the_app_is_told_which_controls_the_plugin_moved_and_where_to() {
        assert_eq!(told_of(&[(16, 0.5), (3, 1.0)]), json!({ "moved": [{ "index": 16, "amount": 0.5 }, { "index": 3, "amount": 1.0 }] }));
    }

    #[test]
    fn the_volume_and_passing_the_effect_by_are_read_from_their_lines() {
        assert_eq!(Request::from_line(r#"{"volume": 0.5}"#), Some(Request::Volume(0.5)));
        assert_eq!(Request::from_line(r#"{"volume": "loud"}"#), None);
        assert_eq!(Request::from_line(r#"{"effect": false}"#), Some(Request::Effect(false)));
        assert_eq!(Request::from_line(r#"{"effect": 0}"#), None);
    }

    #[test]
    fn the_volume_knob_stays_between_silence_and_full() {
        let listening = Listening::as_usual();

        assert_eq!(listening.set_volume(7.0), 1.0);
        assert_eq!(listening.level(), 1.0);
        assert_eq!(listening.set_volume(-1.0), 0.0);
        assert_eq!(listening.level(), 0.0);
        assert_eq!(listening.set_volume(f32::NAN), USUAL_VOLUME);
    }

    #[test]
    fn half_way_round_the_volume_knob_is_a_quarter_of_the_level() {
        let listening = Listening::as_usual();

        listening.set_volume(0.5);

        assert_eq!(listening.level(), 0.25);
    }

    #[test]
    fn only_an_effect_can_be_passed_by() {
        let listening = Listening::as_usual();
        listening.set_volume(1.0);
        listening.hear_the_effect(false);

        assert_eq!(mix_wanted(&listening, false), Mix { level: 1.0, share_of_the_plugin: 0.0 });
        assert_eq!(mix_wanted(&listening, true), Mix { level: 1.0, share_of_the_plugin: 1.0 });
    }

    const TEST_RATE: u32 = 1_000;
    const FED: [[f32; 2]; 2] = [[0.2, -0.2], [0.2, -0.2]];
    const HEARD_PLUGIN: Mix = Mix { level: 0.5, share_of_the_plugin: 1.0 };
    const PASSED_BY: Mix = Mix { level: 0.5, share_of_the_plugin: 0.0 };

    fn carried(way_out: &mut WayOut, wanted: Mix, made: &[[f32; 2]]) -> Vec<[f32; 2]> {
        let mut heard = vec![[9.0; 2]; made.len()];
        way_out.carry(wanted, Sound { fed: &FED[..made.len()], made }, &mut heard);
        heard
    }

    #[test]
    fn sound_is_as_loud_as_the_volume_and_never_louder_than_full_scale() {
        let mut way_out = WayOut::resting_at(HEARD_PLUGIN, TEST_RATE);

        let heard = carried(&mut way_out, HEARD_PLUGIN, &[[0.5, -0.5], [4.0, -4.0]]);

        assert_eq!(heard, [[0.25, -0.25], [1.0, -1.0]]);
    }

    #[test]
    fn an_effect_that_is_passed_by_lets_through_what_it_was_fed() {
        let mut way_out = WayOut::resting_at(PASSED_BY, TEST_RATE);

        let heard = carried(&mut way_out, PASSED_BY, &[[0.9, 0.9], [0.9, 0.9]]);

        assert_eq!(heard, [[0.1, -0.1], [0.1, -0.1]]);
    }

    #[test]
    fn broken_sound_from_the_plugin_is_never_heard_but_what_it_was_fed_still_is_when_it_is_passed_by() {
        let broken = [[0.5, 0.5], [f32::NAN, f32::INFINITY]];

        let heard_with_the_plugin = carried(&mut WayOut::resting_at(HEARD_PLUGIN, TEST_RATE), HEARD_PLUGIN, &broken);
        let heard_passing_it_by = carried(&mut WayOut::resting_at(PASSED_BY, TEST_RATE), PASSED_BY, &broken);

        assert_eq!(heard_with_the_plugin, [[0.0; 2]; 2]);
        assert_eq!(heard_passing_it_by, [[0.1, -0.1], [0.1, -0.1]]);
    }

    #[test]
    fn a_change_is_followed_a_little_with_each_frame_and_not_at_once() {
        let mut glide = Glide::resting_at(0.0, TEST_RATE);

        let steps: Vec<f32> = (0..25).map(|_| glide.towards(1.0)).collect();

        assert!((steps[0] - 0.05).abs() < 1e-6, "one frame moves a twentieth of the way at this rate, not {}", steps[0]);
        assert!(steps.windows(2).all(|pair| pair[1] >= pair[0]));
        assert_eq!(steps[24], 1.0);
    }

    #[test]
    fn the_first_two_channels_carry_the_sound_and_the_rest_stay_silent() {
        let heard = [[0.5, -0.5]];
        let mut four_channels = [9.0f32; 4];
        let mut one_channel = [9.0f32; 1];

        write_frames(&heard, &mut four_channels, 4);
        write_frames(&heard, &mut one_channel, 1);

        assert_eq!(four_channels, [0.5, -0.5, 0.0, 0.0]);
        assert_eq!(one_channel, [0.5]);
    }
}
