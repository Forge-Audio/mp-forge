//! MIDI keyboards and controllers plugged into this computer, standing in for whatever is plugged into the
//! MPC Sample's USB socket or its MIDI IN while an app runs here.
//!
//! Every MIDI input the computer has is listened to, as the device listens at both of its own. What is plugged
//! in is looked at again every little while, so a keyboard plugged in while an app runs is heard too, where
//! the computer says so without being asked again.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use midir::{Ignore, MidiInput, MidiInputConnection};

/// The name other MIDI programs see the listener by.
const NAME_AMONG_MIDI_PROGRAMS: &str = "MP Forge";
const LOOKS_AGAIN_EVERY: Duration = Duration::from_secs(2);

/// What every message that arrives is handed to, as its bytes.
type HandOn = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// A MIDI input that is being listened to, by the name the computer knows it by for as long as it is plugged in.
struct OpenInput {
    id: String,
    _heard_while_this_is_kept: MidiInputConnection<()>,
}

/// The MIDI inputs of this computer, each listened to for as long as it stays plugged in.
struct MidiListener {
    hand_on: HandOn,
    open: Vec<OpenInput>,
}

impl MidiListener {
    fn handing_on(hand_on: HandOn) -> Self {
        Self { hand_on, open: Vec::new() }
    }

    /// Lets go of the inputs that were unplugged and opens the ones plugged in since.
    fn follow_what_is_plugged_in(&mut self) {
        let plugged_in = ids_of_what_is_plugged_in();
        self.open.retain(|open| plugged_in.contains(&open.id));
        for id in not_yet_among(&plugged_in, &self.ids_open()) {
            self.open.extend(self.opened(id));
        }
    }

    fn ids_open(&self) -> Vec<String> {
        self.open.iter().map(|open| open.id.clone()).collect()
    }

    /// Messages that are not played, such as a keyboard's clock and its "still here", are left out by the
    /// input itself.
    fn opened(&self, id: String) -> Option<OpenInput> {
        let mut input = MidiInput::new(NAME_AMONG_MIDI_PROGRAMS).ok()?;
        input.ignore(Ignore::All);
        let port = input.find_port_by_id(&id)?;
        let hand_on = Arc::clone(&self.hand_on);
        let connection = input.connect(&port, NAME_AMONG_MIDI_PROGRAMS, move |_when, message, _| hand_on(message), ()).ok()?;
        Some(OpenInput { id, _heard_while_this_is_kept: connection })
    }
}

fn ids_of_what_is_plugged_in() -> Vec<String> {
    let looking = MidiInput::new(NAME_AMONG_MIDI_PROGRAMS);
    looking.map(|looking| looking.ports().iter().map(|port| port.id()).collect()).unwrap_or_default()
}

fn not_yet_among(plugged_in: &[String], open: &[String]) -> Vec<String> {
    plugged_in.iter().filter(|id| !open.contains(id)).cloned().collect()
}

/// Listens to every MIDI input of this computer from now on, on a thread of its own, handing each message
/// that arrives to `hand_on`.
pub fn keep_listening(hand_on: impl Fn(&[u8]) + Send + Sync + 'static) {
    thread::spawn(move || {
        let mut listener = MidiListener::handing_on(Arc::new(hand_on));
        loop {
            listener.follow_what_is_plugged_in();
            thread::sleep(LOOKS_AGAIN_EVERY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// Makes a MIDI keyboard out of nothing, which the computer shows to every program, and plays a note on
    /// it. It is left out of the usual run because it needs a computer with MIDI to itself.
    #[cfg(unix)]
    #[test]
    #[ignore = "makes a MIDI device on this computer"]
    fn a_keyboard_plugged_in_while_listening_is_heard_note_for_note() {
        use midir::MidiOutput;
        use midir::os::unix::VirtualOutput;
        use std::sync::Mutex;

        let heard = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        keep_listening({ let heard = Arc::clone(&heard); move |message: &[u8]| heard.lock().unwrap().push(message.to_vec()) });

        thread::sleep(Duration::from_millis(500));
        let mut keyboard = MidiOutput::new("workbench test").unwrap().create_virtual("Workbench Test Keys").unwrap();
        thread::sleep(LOOKS_AGAIN_EVERY * 2);
        keyboard.send(&[0x90, 60, 100]).unwrap();
        keyboard.send(&[0x80, 60, 0]).unwrap();
        keyboard.send(&[0xF8]).unwrap();
        thread::sleep(Duration::from_millis(500));

        assert_eq!(*heard.lock().unwrap(), [vec![0x90, 60, 100], vec![0x80, 60, 0]], "its notes arrive and its clock is left out");
    }

    #[test]
    fn only_what_was_plugged_in_since_is_opened() {
        assert_eq!(not_yet_among(&ids(&["keys", "pads"]), &ids(&["keys"])), ids(&["pads"]));
        assert_eq!(not_yet_among(&ids(&["keys"]), &ids(&["keys"])), ids(&[]));
        assert_eq!(not_yet_among(&ids(&[]), &ids(&["keys"])), ids(&[]));
    }
}
