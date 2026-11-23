//! The stream of bytes an MPC's own controls send, read as whole messages.
//!
//! An MPC's pads, knobs, keys and buttons speak MIDI on a port of their own, apart from its MIDI sockets. The
//! bytes arrive in whatever pieces they arrive in, and this puts them together into messages. Which message
//! is which control differs from one MPC to another, and is read by whoever knows that MPC.

use crate::sample_app::{NOTE_OFF, NOTE_ON, PADS};

/// The note each pad sends, from pad 1 to pad 16. They are not in the order of the pads. An MPC Sample and an
/// MPC Key 37 send the same ones.
const NOTES_OF_THE_PADS: [u8; PADS] = [37, 36, 42, 82, 40, 38, 46, 44, 48, 47, 45, 43, 49, 55, 51, 53];

pub(crate) const KIND_OF_MESSAGE: u8 = 0xF0;
pub(crate) const CHANNEL_OF_MESSAGE: u8 = 0x0F;
const FIRST_STATUS_BYTE: u8 = 0x80;
const STARTS_A_LONG_MESSAGE: u8 = 0xF0;
const FIRST_BYTE_SENT_IN_BETWEEN: u8 = 0xF8;
const PROGRAM_CHANGE: u8 = 0xC0;
const CHANNEL_PRESSURE: u8 = 0xD0;

/// One whole message of the controls: what kind it is and on which channel, and its two numbers.
pub(crate) type Message = [u8; 3];

/// Puts the bytes of the controls together into messages, in whatever pieces the bytes arrive.
#[derive(Debug, Default)]
pub(crate) struct MessageReader {
    /// The kind and channel of the message being read. A device may say it once for several messages in a row.
    status: Option<u8>,
    numbers_so_far: Vec<u8>,
    is_inside_a_long_message: bool,
}

impl MessageReader {
    /// The messages that the bytes that arrived since the last time have ended.
    pub(crate) fn read(&mut self, bytes: &[u8]) -> Vec<Message> {
        bytes.iter().filter_map(|byte| self.message_ended_by(*byte)).collect()
    }

    fn message_ended_by(&mut self, byte: u8) -> Option<Message> {
        match byte {
            FIRST_BYTE_SENT_IN_BETWEEN.. => None,
            FIRST_STATUS_BYTE.. => self.start_a_message(byte),
            _ if self.is_inside_a_long_message => None,
            _ => self.take_a_number(byte),
        }
    }

    fn start_a_message(&mut self, status: u8) -> Option<Message> {
        self.is_inside_a_long_message = status == STARTS_A_LONG_MESSAGE;
        self.status = (status < STARTS_A_LONG_MESSAGE).then_some(status);
        self.numbers_so_far.clear();
        None
    }

    fn take_a_number(&mut self, number: u8) -> Option<Message> {
        let status = self.status?;
        self.numbers_so_far.push(number);
        if self.numbers_so_far.len() < numbers_in_a_message_of(status) {
            return None;
        }
        let message = [status, self.numbers_so_far[0], self.numbers_so_far.get(1).copied().unwrap_or(0)];
        self.numbers_so_far.clear();
        Some(message)
    }
}

fn numbers_in_a_message_of(status: u8) -> usize {
    if matches!(status & KIND_OF_MESSAGE, PROGRAM_CHANGE | CHANNEL_PRESSURE) { 1 } else { 2 }
}

/// Whether a note says its control was pressed or let go. None for a message that is not a note.
pub(crate) fn is_pressed(kind: u8, velocity: u8) -> Option<bool> {
    match kind {
        NOTE_ON => Some(velocity > 0),
        NOTE_OFF => Some(false),
        _ => None,
    }
}

/// The pad that sends this note, counted from 1.
pub(crate) fn pad_that_sends(note: u8) -> Option<u8> {
    NOTES_OF_THE_PADS.iter().position(|of_a_pad| *of_a_pad == note).map(|pad| pad as u8 + 1)
}
