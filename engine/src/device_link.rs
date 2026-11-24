//! The link to a real MPC Sample: the device as the screen and the controls of an app that runs here.
//!
//! A small program of MP Forge's on the device carries, and nothing more. What the device's controls send
//! comes here as it is, and rows of the app's picture go back for its screen, with the bytes that set its
//! lights. The app, and all of its sound, stay on this computer.
//!
//! The two talk over the USB-C cable, on the little network the device makes of it. Nobody switches the link
//! on: while an app runs it is looked for every little while, and used when it is there. Nothing is sent to
//! find out whether the cable is in, and nothing is sent to anything that does not first say it is the program.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpStream, UdpSocket};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::sample_app::{Input, SCREEN_HEIGHT, SCREEN_WIDTH, Shown};
use crate::sample_surface::{Lights, SurfaceReader};

/// The address the MPC Sample has on the network it makes of its USB-C cable, as Akai's own guide gives it.
pub(crate) const ADDRESS_OF_THE_DEVICE: Ipv4Addr = Ipv4Addr::new(192, 168, 155, 1);
/// The port MP Forge's program on the device waits at.
pub const PORT_OF_THE_PROGRAM: u16 = 47_373;
/// What the program says first, so that nothing else at that address is taken for it. The number at the end
/// goes up when the two could no longer understand each other.
pub const GREETING: &[u8] = b"MP FORGE LINK 1\n";
/// Starts a message of rows for the screen: the first row and how many, each as two bytes with the low one
/// first, and then the rows.
const ROWS_FOR_THE_SCREEN: u8 = b'P';
/// Starts a message of bytes the program passes on to the device's controls, which is how the lights are set:
/// how many bytes as two bytes with the low one first, and then the bytes.
const BYTES_FOR_THE_CONTROLS: u8 = b'S';

const LOOKS_AGAIN_EVERY: Duration = Duration::from_secs(2);
const WAITS_FOR_AN_ANSWER: Duration = Duration::from_secs(1);
/// About as often as a screen is drawn again.
const SHOWS_EVERY: Duration = Duration::from_millis(33);
const BYTES_A_DOT: usize = 4;
const BYTES_A_ROW: usize = SCREEN_WIDTH * BYTES_A_DOT;
const BYTES_A_PICTURE: usize = BYTES_A_ROW * SCREEN_HEIGHT;
const PARTS_OF_AN_ADDRESS_A_NETWORK_SHARES: usize = 3;

/// What everything the musician does on the device is handed to.
type Tell = Arc<dyn Fn(Input) + Send + Sync>;
/// What the app shows now. None until the app has shown anything.
type SharedShowing = Arc<Mutex<Option<Showing>>>;

/// What a device shows of an app: the picture on its screen, as the kit draws it, and its lights.
#[derive(Clone, PartialEq)]
struct Showing {
    picture: Vec<u8>,
    lights: Lights,
}

/// The link to the device, looked for and kept on a thread of its own for as long as this is kept.
pub struct DeviceLink {
    showing: SharedShowing,
}

impl DeviceLink {
    /// Looks for an MPC Sample on its USB-C cable from now on. Everything done on its controls is handed to `tell`.
    pub fn to_the_mpc_sample(tell: impl Fn(Input) + Send + Sync + 'static) -> DeviceLink {
        DeviceLink::to(SocketAddr::from((ADDRESS_OF_THE_DEVICE, PORT_OF_THE_PROGRAM)), tell)
    }

    /// The same, with the program at another address: one that stands in for the device on this computer.
    pub fn to(program: SocketAddr, tell: impl Fn(Input) + Send + Sync + 'static) -> DeviceLink {
        let showing = SharedShowing::default();
        thread::spawn({
            let showing = Arc::clone(&showing);
            move || keep_linked(program, &showing, &(Arc::new(tell) as Tell))
        });
        DeviceLink { showing }
    }

    /// Has the device show what the app shows from now on: its picture on the screen, and its lights.
    pub fn show(&self, shown: &Shown) {
        let now = Showing { picture: shown.picture.clone(), lights: Lights::of(shown) };
        let mut showing = held(&self.showing);
        if now.picture.len() == BYTES_A_PICTURE && showing.as_ref() != Some(&now) {
            *showing = Some(now);
        }
    }
}

fn held(showing: &Mutex<Option<Showing>>) -> MutexGuard<'_, Option<Showing>> {
    showing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn keep_linked(program: SocketAddr, showing: &SharedShowing, tell: &Tell) {
    loop {
        if let Some(linked) = Linked::to(program) {
            linked.carry(showing, tell);
        }
        thread::sleep(LOOKS_AGAIN_EVERY);
    }
}

/// The program on the device, reached and greeted.
struct Linked {
    program: TcpStream,
}

impl Linked {
    fn to(program: SocketAddr) -> Option<Linked> {
        if !is_on_a_network_of_this_computer(program) {
            return None;
        }
        let mut reached = TcpStream::connect_timeout(&program, WAITS_FOR_AN_ANSWER).ok()?;
        reached.set_read_timeout(Some(WAITS_FOR_AN_ANSWER)).ok()?;
        let mut greeting = [0u8; GREETING.len()];
        reached.read_exact(&mut greeting).ok()?;
        reached.set_read_timeout(None).ok()?;
        reached.set_nodelay(true).ok()?;
        (greeting == GREETING).then_some(Linked { program: reached })
    }

    /// Carries what is touched one way and what the app shows the other, until the program is gone.
    fn carry(mut self, showing: &SharedShowing, tell: &Tell) {
        let is_gone = Arc::new(AtomicBool::new(false));
        let Ok(touched) = self.program.try_clone() else { return };
        thread::spawn({
            let (tell, is_gone) = (Arc::clone(tell), Arc::clone(&is_gone));
            move || {
                hand_on_what_is_touched(touched, &tell);
                is_gone.store(true, Ordering::Relaxed);
            }
        });
        let _ = self.keep_showing(showing, &is_gone);
        let _ = self.program.shutdown(Shutdown::Both);
    }

    /// A device that has just been linked is taken to show nothing yet, so it is sent everything once.
    fn keep_showing(&mut self, showing: &SharedShowing, is_gone: &AtomicBool) -> io::Result<()> {
        let mut on_the_device: Option<Showing> = None;
        while !is_gone.load(Ordering::Relaxed) {
            let now = held(showing).clone();
            if let Some(now) = now.filter(|now| on_the_device.as_ref() != Some(now)) {
                self.program.write_all(&what_brings_up_to(&now, on_the_device.as_ref()))?;
                on_the_device = Some(now);
            }
            thread::sleep(SHOWS_EVERY);
        }
        Ok(())
    }
}

/// The messages that bring a device showing `on_the_device` up to what the app shows now.
fn what_brings_up_to(now: &Showing, on_the_device: Option<&Showing>) -> Vec<u8> {
    let picture_there = on_the_device.map_or(&[][..], |there| &there.picture);
    let rows = rows_that_changed(picture_there, &now.picture).unwrap_or_default();
    let lights = bytes_for_the_controls(&now.lights.bytes_to_set_from(on_the_device.map(|there| &there.lights)));
    [rows, lights].concat()
}

/// The message that has the program pass these bytes on to the device's controls. Nothing when there are none.
fn bytes_for_the_controls(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }
    [&[BYTES_FOR_THE_CONTROLS][..], &(bytes.len() as u16).to_le_bytes(), bytes].concat()
}

/// Whether the address is on a network this computer is itself on, as the device's is while its cable is in.
/// The computer is only asked which of its own addresses it would reach it from: nothing is sent.
pub(crate) fn is_on_a_network_of_this_computer(program: SocketAddr) -> bool {
    let reached_from = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|asking| {
        asking.connect(program)?;
        asking.local_addr()
    });
    match (reached_from.map(|from| from.ip()), program.ip()) {
        (Ok(IpAddr::V4(from)), IpAddr::V4(to)) => network_of(from) == network_of(to),
        _ => false,
    }
}

fn network_of(address: Ipv4Addr) -> [u8; PARTS_OF_AN_ADDRESS_A_NETWORK_SHARES] {
    let parts = address.octets();
    [parts[0], parts[1], parts[2]]
}

fn hand_on_what_is_touched(mut program: TcpStream, tell: &Tell) {
    let mut reader = SurfaceReader::default();
    let mut bytes = [0u8; 256];
    while let Ok(arrived @ 1..) = program.read(&mut bytes) {
        reader.read(&bytes[..arrived]).into_iter().for_each(|touched| tell(touched));
    }
}

/// The message that brings the device's screen up to the picture shown now: the rows from the first that
/// differs to the last, in the screen's own order of colours. None while there is nothing new to show.
fn rows_that_changed(on_the_device: &[u8], shown_now: &[u8]) -> Option<Vec<u8>> {
    let rows = rows_that_differ(on_the_device, shown_now)?;
    let two_bytes = |number: usize| (number as u16).to_le_bytes();
    let mut message = vec![ROWS_FOR_THE_SCREEN];
    message.extend(two_bytes(rows.start));
    message.extend(two_bytes(rows.len()));
    message.extend(shown_now[rows.start * BYTES_A_ROW..rows.end * BYTES_A_ROW].chunks_exact(BYTES_A_DOT).flat_map(as_the_screen_takes_it));
    Some(message)
}

fn rows_that_differ(on_the_device: &[u8], shown_now: &[u8]) -> Option<Range<usize>> {
    if shown_now.len() != BYTES_A_PICTURE {
        return None;
    }
    if on_the_device.len() != BYTES_A_PICTURE {
        return Some(0..SCREEN_HEIGHT);
    }
    let differs = |row: &usize| row_of(on_the_device, *row) != row_of(shown_now, *row);
    let first = (0..SCREEN_HEIGHT).find(differs)?;
    let last = (0..SCREEN_HEIGHT).rfind(differs)?;
    Some(first..last + 1)
}

fn row_of(picture: &[u8], row: usize) -> &[u8] {
    &picture[row * BYTES_A_ROW..(row + 1) * BYTES_A_ROW]
}

/// The kit draws a dot as red, green, blue. The device's screen takes blue, green, red, and a byte it ignores.
fn as_the_screen_takes_it(dot: &[u8]) -> [u8; BYTES_A_DOT] {
    [dot[2], dot[1], dot[0], 0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample_app::PADS;
    use std::net::TcpListener;
    use std::sync::mpsc;

    const HEADER: usize = 5;
    const HEADER_OF_BYTES_FOR_THE_CONTROLS: usize = 3;
    const PAD_ONE_RED: [u8; 12] = [0xF0, 0x47, 0x7F, 0x5C, 0x65, 0x00, 0x04, 0, 127, 0, 0, 0xF7];

    /// A picture of one colour all over, with the first pad lit red and every other light off.
    fn shown_in(dot: [u8; BYTES_A_DOT]) -> Shown {
        let mut pad_colours = [[0u8; 3]; PADS];
        pad_colours[0] = [255, 0, 0];
        Shown { picture: picture_of(dot), pad_colours, lit_buttons: Vec::new(), level_lights: [0; 2] }
    }

    fn showing(shown: &Shown) -> Showing {
        Showing { picture: shown.picture.clone(), lights: Lights::of(shown) }
    }

    fn picture_of(dot: [u8; BYTES_A_DOT]) -> Vec<u8> {
        dot.repeat(SCREEN_WIDTH * SCREEN_HEIGHT)
    }

    fn with_row(mut picture: Vec<u8>, row: usize, dot: [u8; BYTES_A_DOT]) -> Vec<u8> {
        picture[row * BYTES_A_ROW..(row + 1) * BYTES_A_ROW].copy_from_slice(&dot.repeat(SCREEN_WIDTH));
        picture
    }

    #[test]
    fn a_screen_that_has_shown_nothing_is_sent_the_whole_picture_in_its_own_order_of_colours() {
        let message = rows_that_changed(&[], &picture_of([1, 2, 3, 255])).unwrap();

        assert_eq!(message[..HEADER], [b'P', 0, 0, 240, 0]);
        assert_eq!(message.len(), HEADER + BYTES_A_PICTURE);
        assert_eq!(message[HEADER..HEADER + 8], [3, 2, 1, 0, 3, 2, 1, 0]);
    }

    #[test]
    fn after_that_only_the_rows_from_the_first_that_changed_to_the_last_are_sent() {
        let on_the_device = picture_of([0, 0, 0, 255]);
        let shown_now = with_row(with_row(on_the_device.clone(), 10, [9, 9, 9, 255]), 12, [7, 7, 7, 255]);

        let message = rows_that_changed(&on_the_device, &shown_now).unwrap();

        assert_eq!(message[..HEADER], [b'P', 10, 0, 3, 0]);
        assert_eq!(message.len(), HEADER + 3 * BYTES_A_ROW);
        assert_eq!(message[HEADER..HEADER + 4], [9, 9, 9, 0]);
        assert_eq!(message[HEADER + BYTES_A_ROW..HEADER + BYTES_A_ROW + 4], [0, 0, 0, 0], "the row in between goes along as it is");
    }

    #[test]
    fn nothing_is_sent_while_the_picture_stays_as_it_is_or_before_there_is_one() {
        let picture = picture_of([4, 5, 6, 255]);

        assert_eq!(rows_that_changed(&picture, &picture), None);
        assert_eq!(rows_that_changed(&[], &[]), None);
    }

    #[test]
    fn the_row_number_of_the_last_row_fits_in_the_message() {
        let on_the_device = picture_of([0, 0, 0, 255]);
        let shown_now = with_row(on_the_device.clone(), SCREEN_HEIGHT - 1, [1, 1, 1, 255]);

        assert_eq!(rows_that_changed(&on_the_device, &shown_now).unwrap()[..HEADER], [b'P', 239, 0, 1, 0]);
    }

    #[test]
    fn an_address_is_only_reached_for_on_a_network_this_computer_is_on() {
        assert!(is_on_a_network_of_this_computer(SocketAddr::from((Ipv4Addr::LOCALHOST, PORT_OF_THE_PROGRAM))));
        assert_eq!(network_of(Ipv4Addr::new(192, 168, 155, 7)), network_of(ADDRESS_OF_THE_DEVICE));
        assert_ne!(network_of(Ipv4Addr::new(192, 168, 1, 7)), network_of(ADDRESS_OF_THE_DEVICE));
    }

    #[test]
    fn a_device_just_linked_is_sent_the_whole_picture_and_then_every_light() {
        let sent = what_brings_up_to(&showing(&shown_in([1, 2, 3, 255])), None);

        let lights = &sent[HEADER + BYTES_A_PICTURE..];
        assert_eq!(sent[..HEADER], [b'P', 0, 0, 240, 0]);
        assert_eq!(lights[0], b'S');
        assert_eq!(usize::from(u16::from_le_bytes([lights[1], lights[2]])), lights.len() - HEADER_OF_BYTES_FOR_THE_CONTROLS);
        assert_eq!(lights[HEADER_OF_BYTES_FOR_THE_CONTROLS..][..12], PAD_ONE_RED);
    }

    #[test]
    fn a_light_that_changes_under_a_picture_that_stays_is_sent_by_itself() {
        let on_the_device = showing(&shown_in([1, 2, 3, 255]));
        let with_play_lit = showing(&Shown { lit_buttons: vec!["PLAY"], ..shown_in([1, 2, 3, 255]) });

        assert_eq!(what_brings_up_to(&with_play_lit, Some(&on_the_device)), [b'S', 3, 0, 0xB0, 82, 2]);
        assert_eq!(what_brings_up_to(&on_the_device, Some(&on_the_device)), Vec::<u8>::new());
    }

    /// Stands in for the program on the device: greets whoever reaches it, sends what a hit pad sends, and
    /// hands back the picture it is sent and the start of what it is sent for its lights.
    fn stand_in_program(greeting: &'static [u8]) -> (SocketAddr, mpsc::Receiver<Vec<u8>>) {
        let waiting = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = waiting.local_addr().unwrap();
        let (hand_back, sent_here) = mpsc::channel();
        thread::spawn(move || {
            let (mut computer, _) = waiting.accept().unwrap();
            computer.write_all(greeting).unwrap();
            computer.write_all(&[0x99, 37, 127]).unwrap();
            let mut message = vec![0u8; HEADER + BYTES_A_PICTURE + HEADER_OF_BYTES_FOR_THE_CONTROLS + PAD_ONE_RED.len()];
            if computer.read_exact(&mut message).is_ok() {
                hand_back.send(message).unwrap();
            }
        });
        (address, sent_here)
    }

    #[test]
    fn a_program_that_greets_is_shown_the_picture_and_the_lights_and_what_is_touched_on_it_is_told() {
        let (program, sent_to_the_program) = stand_in_program(GREETING);
        let (tell, told) = mpsc::channel();

        let link = DeviceLink::to(program, move |touched| { let _ = tell.send(touched); });
        link.show(&shown_in([10, 20, 30, 255]));

        let message = sent_to_the_program.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(message[..HEADER + 4], [b'P', 0, 0, 240, 0, 30, 20, 10, 0]);
        assert_eq!(message[HEADER + BYTES_A_PICTURE + HEADER_OF_BYTES_FOR_THE_CONTROLS..], PAD_ONE_RED);
        assert_eq!(told.recv_timeout(Duration::from_secs(10)).unwrap(), Input::pad_hit(1, 1.0));
    }

    #[test]
    fn something_else_at_that_address_is_sent_nothing() {
        let (something_else, sent_to_it) = stand_in_program(b"HTTP/1.1 200 OK\n\n");
        let (tell, told) = mpsc::channel();

        let link = DeviceLink::to(something_else, move |touched| { let _ = tell.send(touched); });
        link.show(&shown_in([10, 20, 30, 255]));

        assert!(sent_to_it.recv_timeout(Duration::from_secs(3)).is_err());
        assert!(told.try_recv().is_err());
    }
}
