//! Sending a Sample app to an MPC Sample over MP Forge Link, so that the card never has to leave the device.
//!
//! The link's program on the device has a door for apps, apart from the one the picture and the controls go
//! through. The app's folder, as it would be written to a card, is handed over there a file at a time, and
//! the link writes it where the boot menu looks. Then the link is asked to run it. The musician says yes on
//! the device first, once each time the link starts.
//!
//! Each thing asked is answered with one byte. Whatever the link refuses ends the sending, and the link takes
//! away what had arrived by then.

use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::device_link::{ADDRESS_OF_THE_DEVICE, PORT_OF_THE_PROGRAM, is_on_a_network_of_this_computer};

/// The port the link's program takes apps at.
pub const PORT_FOR_APPS: u16 = 47_375;
/// What the door for apps says first. The number at the end goes up when the two could no longer understand
/// each other.
const GREETING: &[u8] = b"MP FORGE LINK APPS 1\n";
/// What is said back before anything is asked, so that nothing but MP Forge is taken for it.
const SAID_FIRST: &[u8] = b"MP FORGE SENDS AN APP\n";
const BEGIN_AN_APP: u8 = b'B';
const A_FILE_OF_IT: u8 = b'F';
const END_THE_APP: u8 = b'E';
const RUN_AN_APP: u8 = b'R';
const DONE: u8 = b'K';

const WAITS_TO_REACH_THE_DEVICE: Duration = Duration::from_secs(1);
/// The device waits half a minute for the musician to say yes, so its answer is waited for a little longer.
const WAITS_FOR_THE_MUSICIAN: Duration = Duration::from_secs(45);
const WAITS_FOR_THE_CARD: Duration = Duration::from_secs(60);
const WAITS_FOR_AN_ANSWER_AFTER_A_BREAK: Duration = Duration::from_secs(1);

/// Why the link's program on the device would not take or run an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAllowed,
    NameOrPath,
    NameIsTaken,
    NotWritten,
    Damaged,
    NoSuchApp,
    NotOnACard,
    /// An answer this MP Forge does not know, from a link newer than it.
    Unknown(u8),
}

impl Refusal {
    fn said_as(answer: u8) -> Refusal {
        match answer {
            b'N' => Refusal::NotAllowed,
            b'B' => Refusal::NameOrPath,
            b'T' => Refusal::NameIsTaken,
            b'C' => Refusal::NotWritten,
            b'D' => Refusal::Damaged,
            b'S' => Refusal::NoSuchApp,
            b'I' => Refusal::NotOnACard,
            unknown => Refusal::Unknown(unknown),
        }
    }
}

/// Why an app was not sent.
#[derive(Debug)]
pub enum NotSent {
    NoDeviceLinked,
    /// MP Forge Link runs on the device, but it is one from before it could take apps.
    OlderLink,
    Refused(Refusal),
    BrokeOff(io::Error),
    FolderNotRead(io::Error),
}

impl fmt::Display for NotSent {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let words = match self {
            NotSent::NoDeviceLinked => "No MPC Sample is linked. Start MP Forge Link from the boot menu on your MPC Sample and plug its USB-C cable into this computer. Or write the app to a card instead.",
            NotSent::OlderLink => "The MP Forge Link on your MPC Sample is an older one that cannot take apps. Put the new one on its card (Settings, Your MPC Sample), start it again, then send the app.",
            NotSent::Refused(Refusal::NotAllowed) => "The app was not sent: on the MPC Sample, the dial was not pushed to allow it.",
            NotSent::Refused(Refusal::NameIsTaken) => "A folder with the app's name is already on the MPC Sample's card, and MP Forge did not make it. Rename the app, or take that folder off the card.",
            NotSent::Refused(Refusal::NotWritten) => "The MPC Sample's card did not take the app. It may be full, locked or missing. Nothing half-sent is left on it.",
            NotSent::Refused(Refusal::Damaged) => "The app did not arrive whole, so nothing was put on the card. Send it again.",
            NotSent::Refused(Refusal::NoSuchApp) => "The app is on the card but could not be started from there. Pick it in the boot menu instead.",
            NotSent::Refused(Refusal::NotOnACard) => "MP Forge Link on your MPC Sample is not running from an SD card, so it takes no apps.",
            NotSent::Refused(Refusal::NameOrPath) => "MP Forge Link would not take the app's name or one of its files.",
            NotSent::Refused(Refusal::Unknown(_)) => "MP Forge Link on your MPC Sample is newer than this MP Forge and said something it does not understand. Update MP Forge.",
            NotSent::BrokeOff(cause) => return write!(formatter, "The link to the MPC Sample broke off while the app was sent ({cause}). Nothing half-sent is left on the card."),
            NotSent::FolderNotRead(cause) => return write!(formatter, "The app's files could not be read: {cause}"),
        };
        write!(formatter, "{words}")
    }
}

/// Where the link's program is reached: its door for apps, and the door it links at, which tells an older
/// link from no link at all.
#[derive(Debug, Clone, Copy)]
pub struct Doors {
    for_apps: SocketAddr,
    for_linking: SocketAddr,
}

impl Doors {
    /// The doors of an MPC Sample on its USB-C cable.
    pub fn of_the_mpc_sample() -> Doors {
        Doors { for_apps: SocketAddr::from((ADDRESS_OF_THE_DEVICE, PORT_FOR_APPS)), for_linking: SocketAddr::from((ADDRESS_OF_THE_DEVICE, PORT_OF_THE_PROGRAM)) }
    }

    /// The doors of a program that stands in for the device at another address.
    pub fn at(for_linking: SocketAddr, port_for_apps: u16) -> Doors {
        Doors { for_apps: SocketAddr::new(for_linking.ip(), port_for_apps), for_linking }
    }
}

/// Looks whether the link's program is there to take an app, before any time is spent on getting one ready.
/// Nothing is asked of it, so nothing changes on the device.
pub fn look_for_the_door(doors: &Doors) -> Result<(), NotSent> {
    DoorForApps::reached(doors).map(|_| ())
}

/// Sends the app's folder, as it was written for the boot menu, to the device behind `doors`, and has the
/// link run the app once it is on the card.
pub fn send_and_run(doors: &Doors, folder: &Path) -> Result<(), NotSent> {
    let name = folder.file_name().and_then(|name| name.to_str()).ok_or_else(|| NotSent::FolderNotRead(io::Error::other("the app's folder has no name")))?;
    let files = files_of(folder).map_err(NotSent::FolderNotRead)?;
    let mut door = DoorForApps::reached(doors)?;
    door.ask(&naming(BEGIN_AN_APP, name), WAITS_FOR_THE_MUSICIAN)?;
    for within_the_app in &files {
        let bytes = fs::read(folder.join(within_the_app)).map_err(NotSent::FolderNotRead)?;
        door.ask(&file_handed_over(within_the_app, &bytes), WAITS_FOR_THE_CARD)?;
    }
    door.ask(&[END_THE_APP], WAITS_FOR_THE_CARD)?;
    door.ask(&naming(RUN_AN_APP, name), WAITS_FOR_THE_MUSICIAN)
}

/// The link's door for apps, reached and greeted.
struct DoorForApps {
    link: TcpStream,
}

impl DoorForApps {
    fn reached(doors: &Doors) -> Result<DoorForApps, NotSent> {
        if !is_on_a_network_of_this_computer(doors.for_apps) {
            return Err(NotSent::NoDeviceLinked);
        }
        let Ok(mut link) = TcpStream::connect_timeout(&doors.for_apps, WAITS_TO_REACH_THE_DEVICE) else { return Err(why_no_door_for_apps(doors)) };
        let mut greeting = [0u8; GREETING.len()];
        let is_greeted = link.set_read_timeout(Some(WAITS_TO_REACH_THE_DEVICE)).and_then(|()| link.read_exact(&mut greeting)).is_ok();
        if !is_greeted || greeting != GREETING {
            return Err(NotSent::NoDeviceLinked);
        }
        link.write_all(SAID_FIRST).map_err(NotSent::BrokeOff)?;
        Ok(DoorForApps { link })
    }

    /// Asks one thing and waits for its answer. A link that stops listening in the middle of a file has
    /// usually said why first, so its answer is looked for before the break is reported.
    fn ask(&mut self, asked: &[u8], patience: Duration) -> Result<(), NotSent> {
        match self.link.write_all(asked) {
            Ok(()) => self.answer_within(patience).map_err(NotSent::BrokeOff)?,
            Err(broke_off) => Err(self.answer_within(WAITS_FOR_AN_ANSWER_AFTER_A_BREAK).ok().and_then(Result::err).unwrap_or(NotSent::BrokeOff(broke_off))),
        }
    }

    fn answer_within(&mut self, patience: Duration) -> io::Result<Result<(), NotSent>> {
        let mut answer = [0u8; 1];
        self.link.set_read_timeout(Some(patience))?;
        self.link.read_exact(&mut answer)?;
        Ok(if answer[0] == DONE { Ok(()) } else { Err(NotSent::Refused(Refusal::said_as(answer[0]))) })
    }
}

/// With no door for apps, something at the door for linking is a link from before it could take apps.
fn why_no_door_for_apps(doors: &Doors) -> NotSent {
    match TcpStream::connect_timeout(&doors.for_linking, WAITS_TO_REACH_THE_DEVICE) {
        Ok(_) => NotSent::OlderLink,
        Err(_) => NotSent::NoDeviceLinked,
    }
}

/// What begins an app, or has one run: the kind of asking, how long the name is, and the name.
fn naming(asking: u8, name: &str) -> Vec<u8> {
    [&[asking, name.len() as u8][..], name.as_bytes()].concat()
}

/// One file handed over: how long its path is, the path, its size, the sum of its bytes, and the bytes.
fn file_handed_over(within_the_app: &str, bytes: &[u8]) -> Vec<u8> {
    let mut handed_over = vec![A_FILE_OF_IT];
    handed_over.extend((within_the_app.len() as u16).to_le_bytes());
    handed_over.extend(within_the_app.as_bytes());
    handed_over.extend((bytes.len() as u32).to_le_bytes());
    handed_over.extend(sum_of(bytes).to_le_bytes());
    handed_over.extend(bytes);
    handed_over
}

/// The bytes of a file added up the way the link adds them up as they arrive.
fn sum_of(bytes: &[u8]) -> u32 {
    bytes.iter().fold(2_166_136_261u32, |sum, byte| (sum ^ u32::from(*byte)).wrapping_mul(16_777_619))
}

/// The files of the app's folder, each by its path within the folder with `/` between its steps, in the
/// order of their names. An app's files lie in its folder or one folder down.
fn files_of(folder: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for found in entries_of(folder)? {
        let name = name_of(&found)?;
        if found.is_dir() {
            for further_in in entries_of(&found)?.iter().filter(|further_in| further_in.is_file()) {
                files.push(format!("{name}/{}", name_of(further_in)?));
            }
        } else {
            files.push(name);
        }
    }
    files.sort();
    Ok(files)
}

fn entries_of(folder: &Path) -> io::Result<Vec<PathBuf>> {
    fs::read_dir(folder)?.map(|entry| entry.map(|entry| entry.path())).collect()
}

fn name_of(file: &Path) -> io::Result<String> {
    let name = file.file_name().and_then(|name| name.to_str());
    name.map(str::to_string).ok_or_else(|| io::Error::other(format!("{} has a name that cannot be sent", file.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::mpsc;
    use std::thread;

    /// An app's folder with a program, a file the boot menu reads, and one sound.
    fn folder_of_an_app() -> (TemporaryFolder, PathBuf) {
        let holder = TemporaryFolder::create("app-sending");
        let folder = holder.0.join("Night Bus");
        fs::create_dir_all(folder.join("sounds")).unwrap();
        fs::write(folder.join("app"), b"the program").unwrap();
        fs::write(folder.join("sampleboot.txt"), b"Night Bus").unwrap();
        fs::write(folder.join("sounds/kick.wav"), b"boom").unwrap();
        (holder, folder)
    }

    /// Stands in for the link's door for apps: greets, hears the first line, then answers each thing asked
    /// with the next of `answers`, and hands back everything it was sent once the computer has gone.
    fn stand_in_door(answers: &'static [u8]) -> (Doors, mpsc::Receiver<Vec<u8>>) {
        let waiting = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let doors = Doors::at(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)), waiting.local_addr().unwrap().port());
        let (hand_back, sent_here) = mpsc::channel();
        thread::spawn(move || {
            let (mut computer, _) = waiting.accept().unwrap();
            computer.write_all(GREETING).unwrap();
            let mut said_first = [0u8; SAID_FIRST.len()];
            computer.read_exact(&mut said_first).unwrap();
            let mut sent = said_first.to_vec();
            for answer in answers {
                sent.extend(one_thing_asked(&mut computer));
                computer.write_all(&[*answer]).unwrap();
            }
            hand_back.send(sent).unwrap();
        });
        (doors, sent_here)
    }

    /// Reads one thing asked, as long as its own lengths say it is.
    fn one_thing_asked(computer: &mut TcpStream) -> Vec<u8> {
        let mut read = |count: usize| {
            let mut bytes = vec![0u8; count];
            computer.read_exact(&mut bytes).unwrap();
            bytes
        };
        let kind = read(1);
        let rest = match kind[0] {
            BEGIN_AN_APP | RUN_AN_APP => {
                let length = read(1);
                [length.clone(), read(usize::from(length[0]))].concat()
            }
            A_FILE_OF_IT => {
                let path_length = read(2);
                let path = read(usize::from(u16::from_le_bytes([path_length[0], path_length[1]])));
                let size_and_sum = read(8);
                let size = u32::from_le_bytes([size_and_sum[0], size_and_sum[1], size_and_sum[2], size_and_sum[3]]) as usize;
                [path_length, path, size_and_sum.clone(), read(size)].concat()
            }
            _ => Vec::new(),
        };
        [kind, rest].concat()
    }

    #[test]
    fn an_app_is_begun_handed_over_a_file_at_a_time_ended_and_then_run() {
        let (_holder, folder) = folder_of_an_app();
        let (doors, sent_to_the_door) = stand_in_door(b"KKKKKK");

        send_and_run(&doors, &folder).unwrap();

        let sent = sent_to_the_door.recv_timeout(Duration::from_secs(10)).unwrap();
        let expected = [
            SAID_FIRST,
            b"B\x09Night Bus",
            b"F\x03\x00app\x0b\x00\x00\x00",
            &sum_of(b"the program").to_le_bytes(),
            b"the program",
            b"F\x0e\x00sampleboot.txt\x09\x00\x00\x00",
            &sum_of(b"Night Bus").to_le_bytes(),
            b"Night Bus",
            b"F\x0f\x00sounds/kick.wav\x04\x00\x00\x00",
            &sum_of(b"boom").to_le_bytes(),
            b"boom",
            b"E",
            b"R\x09Night Bus",
        ]
        .concat();
        assert_eq!(String::from_utf8_lossy(&sent), String::from_utf8_lossy(&expected));
    }

    #[test]
    fn what_the_link_refuses_ends_the_sending_and_is_said_in_the_musicians_words() {
        let (_holder, folder) = folder_of_an_app();
        let (doors, sent_to_the_door) = stand_in_door(b"N");

        let refused = send_and_run(&doors, &folder).unwrap_err();

        assert!(matches!(refused, NotSent::Refused(Refusal::NotAllowed)), "{refused:?}");
        assert!(refused.to_string().contains("the dial was not pushed"));
        assert_eq!(sent_to_the_door.recv_timeout(Duration::from_secs(10)).unwrap(), [SAID_FIRST, b"B\x09Night Bus"].concat(), "no file follows a refusal");
    }

    #[test]
    fn every_answer_of_the_link_is_known_by_its_letter() {
        let known = b"NBTCDSI".map(Refusal::said_as);

        assert_eq!(known, [Refusal::NotAllowed, Refusal::NameOrPath, Refusal::NameIsTaken, Refusal::NotWritten, Refusal::Damaged, Refusal::NoSuchApp, Refusal::NotOnACard]);
        assert_eq!(Refusal::said_as(b'?'), Refusal::Unknown(b'?'));
    }

    #[test]
    fn a_link_that_only_links_is_an_older_one_and_nothing_there_at_all_is_no_device() {
        let (_holder, folder) = folder_of_an_app();
        let only_links = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let nothing_waits_here = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap();
        let an_older_link = Doors::at(only_links.local_addr().unwrap(), nothing_waits_here.port());
        let no_link_at_all = Doors::at(nothing_waits_here, nothing_waits_here.port());

        assert!(matches!(send_and_run(&an_older_link, &folder), Err(NotSent::OlderLink)));
        assert!(matches!(send_and_run(&no_link_at_all, &folder), Err(NotSent::NoDeviceLinked)));
    }

    #[test]
    fn something_else_at_the_door_for_apps_is_sent_nothing() {
        let (_holder, folder) = folder_of_an_app();
        let something_else = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let doors = Doors::at(something_else.local_addr().unwrap(), something_else.local_addr().unwrap().port());
        let heard = thread::spawn(move || {
            let (mut computer, _) = something_else.accept().unwrap();
            computer.write_all(b"HTTP/1.1 200 OK\r\n\r\n not a link").unwrap();
            let mut sent = Vec::new();
            let _ = computer.read_to_end(&mut sent);
            sent
        });

        let not_sent = send_and_run(&doors, &folder);

        assert!(matches!(not_sent, Err(NotSent::NoDeviceLinked)));
        assert_eq!(heard.join().unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn the_files_of_an_app_are_named_by_their_paths_within_it() {
        let (_holder, folder) = folder_of_an_app();

        assert_eq!(files_of(&folder).unwrap(), ["app", "sampleboot.txt", "sounds/kick.wav"]);
    }
}
