//! The MPC Sample's own controls, read from the bytes they send, and its lights, set by the bytes they are sent.
//!
//! The device's pads, knobs, fader, dial and buttons speak MIDI on a port of their own, apart from its MIDI
//! sockets. This reads that stream of bytes as the things an app is told of, and writes the bytes that set the
//! lights of the pads and the buttons. Which number is which control comes from public notes by people who
//! measured their own devices. The messages that set the lights come from the same notes, whose writers found
//! them by watching what the controls are sent.

use crate::controls_stream::{CHANNEL_OF_MESSAGE, KIND_OF_MESSAGE, Message, MessageReader, is_pressed, pad_that_sends};
use crate::sample_app::{BUTTONS, CONTROLLER, HARDEST_HIT, Input, PADS, Shown};

/// The pads send notes on a channel of their own, so a button's note is never taken for a pad's.
const CHANNEL_OF_THE_PADS: u8 = 10;
const CHANNEL_OF_THE_OTHER_CONTROLS: u8 = 1;
/// The note each button sends, in the order the kit counts the buttons.
const NOTES_OF_THE_BUTTONS: [u8; BUTTONS.len()] = [12, 13, 14, 6, 7, 1, 0, 49, 35, 9, 11, 75, 43, 74, 40, 36, 53, 55, 54, 73, 80, 81, 82, 111];
const CONTROLLER_OF_THE_FIRST_KNOB: u8 = 16;
const CONTROLLER_OF_THE_LAST_KNOB: u8 = 18;
const CONTROLLER_OF_THE_FADER: u8 = 20;
const CONTROLLER_OF_THE_DIAL: u8 = 100;
/// The dial says how many clicks it was turned: up to this many to the right as they are, and to the left
/// counted down from 128.
const MOST_CLICKS_TO_THE_RIGHT: u8 = 63;
const FULLY_TURNED: f32 = 127.0;

/// What a pad's colour is sent between, and what it is sent after. The pad is counted from 0, and each part of
/// its colour goes up to 127.
const STARTS_A_PAD_COLOUR: [u8; 7] = [0xF0, 0x47, 0x7F, 0x5C, 0x65, 0x00, 0x04];
const ENDS_A_PAD_COLOUR: u8 = 0xF7;
const BRIGHTEST_AN_APP_SAYS: f32 = 255.0;
const BRIGHTEST_A_PAD_TAKES: f32 = 127.0;
/// How much faster than its number the light of a screen falls: at half the number it gives about a fifth.
const FALL_OF_A_SCREENS_LIGHT: f32 = 2.2;
/// A button's light is set by a controller with the number of the button's note.
const SETS_A_BUTTON_LIGHT: u8 = CONTROLLER;
const LIGHT_ON: u8 = 2;
const LIGHT_OFF: u8 = 0;
const BUTTONS_WITHOUT_A_LIGHT: [&str; 7] = ["B1", "B2", "B3", "SHIFT", "MINUS", "PLUS", "ENCODER"];

/// Reads the bytes of the controls as they arrive, in whatever pieces they arrive in.
#[derive(Debug, Default)]
pub struct SurfaceReader {
    messages: MessageReader,
}

impl SurfaceReader {
    /// What the musician did, going by the bytes that arrived since the last time.
    pub fn read(&mut self, bytes: &[u8]) -> Vec<Input> {
        self.messages.read(bytes).into_iter().filter_map(told_by).collect()
    }
}

fn told_by([status, number, value]: Message) -> Option<Input> {
    let kind = status & KIND_OF_MESSAGE;
    match (status & CHANNEL_OF_MESSAGE) + 1 {
        CHANNEL_OF_THE_PADS => told_by_a_pad(kind, number, value),
        CHANNEL_OF_THE_OTHER_CONTROLS if kind == CONTROLLER => told_by_something_turned(number, value),
        CHANNEL_OF_THE_OTHER_CONTROLS => told_by_a_button(kind, number, value),
        _ => None,
    }
}

/// How hard a pad is pressed while it is held is left out: the kit tells an app of the hit and the letting go.
fn told_by_a_pad(kind: u8, note: u8, velocity: u8) -> Option<Input> {
    let pad = i32::from(pad_that_sends(note)?);
    match is_pressed(kind, velocity)? {
        true => Some(Input::pad_hit(pad, f32::from(velocity) / HARDEST_HIT)),
        false => Some(Input::pad_let_go(pad)),
    }
}

fn told_by_a_button(kind: u8, note: u8, velocity: u8) -> Option<Input> {
    let button = NOTES_OF_THE_BUTTONS.iter().position(|of_a_button| *of_a_button == note)?;
    Input::button(BUTTONS[button], is_pressed(kind, velocity)?)
}

/// The main volume knob is left out: an app's sound is heard on the computer, at the computer's own volume.
fn told_by_something_turned(controller: u8, value: u8) -> Option<Input> {
    let stands = f32::from(value) / FULLY_TURNED;
    match controller {
        CONTROLLER_OF_THE_FIRST_KNOB..=CONTROLLER_OF_THE_LAST_KNOB => Some(Input::knob_turned(i32::from(controller - CONTROLLER_OF_THE_FIRST_KNOB) + 1, stands)),
        CONTROLLER_OF_THE_FADER => Some(Input::fader_moved(stands)),
        CONTROLLER_OF_THE_DIAL => clicks_turned(value).map(Input::encoder_turned),
        _ => None,
    }
}

fn clicks_turned(value: u8) -> Option<i32> {
    match value {
        0 => None,
        1..=MOST_CLICKS_TO_THE_RIGHT => Some(i32::from(value)),
        _ => Some(i32::from(value) - 128),
    }
}

// ---------------------------------------------------------------------------
// The lights
// ---------------------------------------------------------------------------

/// The lights an app sets: the colour of each pad, and whether the light of each button is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lights {
    pad_colours: [[u8; 3]; PADS],
    lit_buttons: [bool; BUTTONS.len()],
}

impl Lights {
    pub fn of(shown: &Shown) -> Lights {
        Lights { pad_colours: shown.pad_colours, lit_buttons: BUTTONS.map(|button| shown.lit_buttons.contains(&button)) }
    }

    /// The bytes that bring the device's lights to these, when they stand as `on_the_device` says: only the
    /// lights that differ are set, and all of them when nothing is known of how they stand.
    pub fn bytes_to_set_from(&self, on_the_device: Option<&Lights>) -> Vec<u8> {
        let pads = (0..PADS).filter(|pad| on_the_device.is_none_or(|there| there.pad_colours[*pad] != self.pad_colours[*pad]));
        let buttons = (0..BUTTONS.len()).filter(|button| on_the_device.is_none_or(|there| there.lit_buttons[*button] != self.lit_buttons[*button]));
        pads.flat_map(|pad| sets_the_colour_of(pad, self.pad_colours[pad])).chain(buttons.flat_map(|button| sets_the_light_of(button, self.lit_buttons[button]))).collect()
    }
}

fn sets_the_colour_of(pad: usize, colour: [u8; 3]) -> Vec<u8> {
    STARTS_A_PAD_COLOUR.into_iter().chain([pad as u8]).chain(as_a_pad_lights_it(colour)).chain([ENDS_A_PAD_COLOUR]).collect()
}

/// An app says a colour as a screen shows it, where a part at half its number gives far less than half the
/// light. A pad's light gives light in step with its number, so the same numbers would wash a colour out
/// towards white. Each part is therefore weighed against the strongest as a screen weighs it, which keeps the
/// colour, and the strongest is sent in step with its own number, which keeps a dim pad lit.
fn as_a_pad_lights_it(colour: [u8; 3]) -> [u8; 3] {
    let strongest = f32::from(colour.into_iter().max().unwrap_or(0));
    if strongest == 0.0 {
        return [0; 3];
    }
    let brightness = strongest / BRIGHTEST_AN_APP_SAYS * BRIGHTEST_A_PAD_TAKES;
    colour.map(|part| ((f32::from(part) / strongest).powf(FALL_OF_A_SCREENS_LIGHT) * brightness).round() as u8)
}

fn sets_the_light_of(button: usize, is_lit: bool) -> Vec<u8> {
    if BUTTONS_WITHOUT_A_LIGHT.contains(&BUTTONS[button]) {
        return Vec::new();
    }
    vec![SETS_A_BUTTON_LIGHT, NOTES_OF_THE_BUTTONS[button], if is_lit { LIGHT_ON } else { LIGHT_OFF }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample_app::{SCREEN_HEIGHT, SCREEN_WIDTH};

    fn lights(pad_colours: [[u8; 3]; PADS], lit_buttons: &[&'static str]) -> Lights {
        Lights::of(&Shown { picture: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4], pad_colours, lit_buttons: lit_buttons.to_vec() })
    }

    #[test]
    fn a_device_whose_lights_are_not_known_is_sent_every_pad_and_every_button_that_has_a_light() {
        let mut pad_colours = [[0u8; 3]; PADS];
        pad_colours[15] = [255, 128, 1];

        let bytes = lights(pad_colours, &["PLAY"]).bytes_to_set_from(None);

        let (pads, buttons) = bytes.split_at(PADS * 12);
        assert_eq!(pads[..12], [0xF0, 0x47, 0x7F, 0x5C, 0x65, 0x00, 0x04, 0, 0, 0, 0, 0xF7]);
        assert_eq!(pads[15 * 12..], [0xF0, 0x47, 0x7F, 0x5C, 0x65, 0x00, 0x04, 15, 127, 28, 0, 0xF7]);
        assert_eq!(buttons.len(), (BUTTONS.len() - BUTTONS_WITHOUT_A_LIGHT.len()) * 3);
        assert!(buttons.chunks(3).any(|button| button == [0xB0, 82, 2]), "PLAY is lit");
        assert!(buttons.chunks(3).any(|button| button == [0xB0, 81, 0]), "STOP is not");
        assert!(!buttons.chunks(3).any(|button| button[1] == 49), "SHIFT has no light to set");
    }

    #[test]
    fn after_that_only_the_lights_that_changed_are_sent() {
        let mut pad_colours = [[0u8; 3]; PADS];
        let on_the_device = lights(pad_colours, &["PLAY"]);
        pad_colours[2] = [0, 200, 0];

        let bytes = lights(pad_colours, &["STOP"]).bytes_to_set_from(Some(&on_the_device));

        assert_eq!(bytes, [0xF0, 0x47, 0x7F, 0x5C, 0x65, 0x00, 0x04, 2, 0, 100, 0, 0xF7, 0xB0, 81, 2, 0xB0, 82, 0]);
        assert_eq!(on_the_device.bytes_to_set_from(Some(&on_the_device)), Vec::<u8>::new());
    }

    #[test]
    fn a_pad_is_lit_in_the_colour_a_screen_shows_and_not_washed_out_towards_white() {
        const AMBER: [u8; 3] = [255, 170, 50];
        const THE_SAME_AMBER_DIMMED: [u8; 3] = [33, 22, 7];

        assert_eq!(as_a_pad_lights_it(AMBER), [127, 52, 4], "the weaker parts fall away as they do on a screen");
        assert_eq!(as_a_pad_lights_it(THE_SAME_AMBER_DIMMED), [16, 7, 1], "a dim pad stays lit, in the same colour");
        assert_eq!(as_a_pad_lights_it([255, 255, 255]), [127, 127, 127]);
        assert_eq!(as_a_pad_lights_it([0, 0, 0]), [0, 0, 0]);
    }

    #[test]
    fn a_button_without_a_light_being_lit_sends_nothing() {
        let dark = lights([[0; 3]; PADS], &[]);

        assert_eq!(lights([[0; 3]; PADS], &["SHIFT", "ENCODER"]).bytes_to_set_from(Some(&dark)), Vec::<u8>::new());
    }

    fn read(bytes: &[u8]) -> Vec<Input> {
        SurfaceReader::default().read(bytes)
    }

    #[test]
    fn a_pad_is_known_by_its_own_note_and_not_by_its_place_in_a_row_of_notes() {
        assert_eq!(read(&[0x99, 37, 127]), [Input::pad_hit(1, 1.0)]);
        assert_eq!(read(&[0x99, 36, 64]), [Input::pad_hit(2, 64.0 / 127.0)]);
        assert_eq!(read(&[0x99, 53, 1]), [Input::pad_hit(16, 1.0 / 127.0)]);
        assert_eq!(read(&[0x99, 50, 100]), [], "a note no pad sends");
    }

    #[test]
    fn a_pad_is_let_go_however_the_device_says_so() {
        assert_eq!(read(&[0x89, 37, 0]), [Input::pad_let_go(1)]);
        assert_eq!(read(&[0x99, 37, 0]), [Input::pad_let_go(1)]);
    }

    #[test]
    fn how_hard_a_held_pad_is_pressed_is_left_out() {
        assert_eq!(read(&[0xA9, 37, 90]), []);
    }

    #[test]
    fn a_button_is_told_from_a_pad_that_sends_the_same_note() {
        assert_eq!(read(&[0x90, 82, 127]), [Input::button("PLAY", true).unwrap()]);
        assert_eq!(read(&[0x99, 82, 127]), [Input::pad_hit(4, 1.0)]);
        assert_eq!(read(&[0x80, 82, 0]), [Input::button("PLAY", false).unwrap()]);
    }

    #[test]
    fn every_button_has_a_note_of_its_own() {
        let mut notes = NOTES_OF_THE_BUTTONS.to_vec();
        notes.sort_unstable();
        notes.dedup();

        assert_eq!(notes.len(), BUTTONS.len());
        assert_eq!(read(&[0x90, 111, 127]), [Input::button("ENCODER", true).unwrap()]);
        assert_eq!(read(&[0x90, 12, 127, 0x90, 14, 127]), [Input::button("B1", true).unwrap(), Input::button("B3", true).unwrap()]);
        assert_eq!(read(&[0x90, 55, 127, 0x90, 54, 127]), [Input::button("MINUS", true).unwrap(), Input::button("PLUS", true).unwrap()]);
    }

    #[test]
    fn the_knobs_and_the_fader_say_where_they_stand() {
        assert_eq!(read(&[0xB0, 16, 0]), [Input::knob_turned(1, 0.0)]);
        assert_eq!(read(&[0xB0, 18, 127]), [Input::knob_turned(3, 1.0)]);
        assert_eq!(read(&[0xB0, 20, 127]), [Input::fader_moved(1.0)]);
        assert_eq!(read(&[0xB0, 19, 100]), [], "the main volume knob is the computer's business");
    }

    #[test]
    fn the_dial_says_how_many_clicks_it_was_turned_and_which_way() {
        assert_eq!(read(&[0xB0, 100, 1]), [Input::encoder_turned(1)]);
        assert_eq!(read(&[0xB0, 100, 127]), [Input::encoder_turned(-1)]);
        assert_eq!(read(&[0xB0, 100, 3]), [Input::encoder_turned(3)]);
        assert_eq!(read(&[0xB0, 100, 125]), [Input::encoder_turned(-3)]);
        assert_eq!(read(&[0xB0, 100, 0]), [], "a dial that was not turned");
    }

    #[test]
    fn messages_are_read_whole_however_the_bytes_arrive() {
        let mut reader = SurfaceReader::default();

        let from_half_a_message = reader.read(&[0x99, 37]);
        let from_the_rest = reader.read(&[127, 36, 100]);

        assert_eq!(from_half_a_message, []);
        assert_eq!(from_the_rest, [Input::pad_hit(1, 1.0), Input::pad_hit(2, 100.0 / 127.0)], "the kind of message is said once for both");
    }

    #[test]
    fn what_is_not_a_control_is_read_past() {
        let a_long_message = [0xF0, 0x47, 0x7F, 0x5C, 37, 127, 0xF7];
        let a_clock_inside_a_hit = [0x99, 37, 0xF8, 127];

        assert_eq!(read(&[&a_long_message[..], &[0x99, 37, 127]].concat()), [Input::pad_hit(1, 1.0)]);
        assert_eq!(read(&a_clock_inside_a_hit), [Input::pad_hit(1, 1.0)]);
        assert_eq!(read(&[37, 127]), [], "numbers with no kind of message before them");
    }
}
