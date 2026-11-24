//! The blacksmith on the MPC Sample's screen, for as long as an app is out of reach: while the AI writes it and
//! while it is built and checked.
//!
//! It is shown the way an app is: by a program of its own, which writes what the screen shows on standard
//! output for the drawing of the device, and sends the same to a real MPC Sample on its USB-C cable. So the
//! drawing and the device show the blacksmith alike. Every light is dark meanwhile, and nothing touched on the
//! device does anything.
//!
//! The blacksmith is the strip of frames the window shows over a plugin's view, handed over as it is.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use resvg::tiny_skia::Pixmap;

use crate::device_link::DeviceLink;
use crate::sample_app::{PADS, SCREEN_HEIGHT, SCREEN_WIDTH, Shown};
use crate::sample_player::{as_bytes, write_out};

/// The command that starts the showing of the blacksmith. `KEEPS_STILL` after it holds him still.
pub const SHOW_SMITH_COMMAND: &str = "show-smith-on-sample";
pub const KEEPS_STILL: &str = "still";

const FRAMES: usize = 15;
const FRAME_LASTS: Duration = Duration::from_millis(83);
const FRAME_WHEN_STILL: usize = 1;
/// The blacksmith is drawn small, so each of his dots is shown as a square this many dots of the screen wide.
const TIMES_BIGGER: usize = 3;
const BYTES_A_DOT: usize = 4;
const GROUND: [u8; BYTES_A_DOT] = [17, 18, 20, 255];
const SEEN_THROUGH_FULLY: u16 = 255;

/// Shows the blacksmith until the app that started this closes its end. `strip_png` holds his frames side by
/// side. A blacksmith who keeps still is shown once, in one frame.
pub fn show_on_standard_io(strip_png: &[u8], keeps_still: bool) -> Result<(), String> {
    let pictures = pictures_of(strip_png)?;
    thread::spawn(move || keep_showing(&pictures, keeps_still));
    io::stdin().lines().take_while(Result::is_ok).for_each(drop);
    Ok(())
}

fn keep_showing(pictures: &[Vec<u8>], keeps_still: bool) {
    let device = DeviceLink::to_the_mpc_sample(|_touched| {});
    let started = Instant::now();
    let mut last_shown = None;
    loop {
        let frame = if keeps_still { FRAME_WHEN_STILL } else { frame_after(started.elapsed()) };
        if last_shown != Some(frame) {
            let shown = with_every_light_dark(&pictures[frame % pictures.len()]);
            device.show(&shown);
            if write_out(&as_bytes(&shown, false)).is_err() {
                return;
            }
            last_shown = Some(frame);
        }
        thread::sleep(FRAME_LASTS);
    }
}

fn frame_after(elapsed: Duration) -> usize {
    (elapsed.as_millis() / FRAME_LASTS.as_millis()) as usize % FRAMES
}

fn with_every_light_dark(picture: &[u8]) -> Shown {
    Shown { picture: picture.to_vec(), pad_colours: [[0; 3]; PADS], lit_buttons: Vec::new(), level_lights: [0; 2] }
}

/// Each frame of the strip as a picture for the Sample's screen.
fn pictures_of(strip_png: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let strip = Pixmap::decode_png(strip_png).map_err(|error| format!("The blacksmith's picture could not be read: {error}"))?;
    Ok((0..FRAMES).map(|frame| picture_of(&strip, frame)).collect())
}

/// One frame in the middle of the screen, made bigger, on the dark ground the window shows him on.
fn picture_of(strip: &Pixmap, frame: usize) -> Vec<u8> {
    let (across, down) = (strip.width() as usize / FRAMES, strip.height() as usize);
    let (left, top) = (SCREEN_WIDTH.saturating_sub(across * TIMES_BIGGER) / 2, SCREEN_HEIGHT.saturating_sub(down * TIMES_BIGGER) / 2);
    let dot_at = |x: usize, y: usize| {
        let is_on_him = x >= left && y >= top && (x - left) / TIMES_BIGGER < across && (y - top) / TIMES_BIGGER < down;
        if is_on_him { over_the_ground(dot_of(strip, frame * across + (x - left) / TIMES_BIGGER, (y - top) / TIMES_BIGGER)) } else { GROUND }
    };
    (0..SCREEN_HEIGHT).flat_map(|y| (0..SCREEN_WIDTH).flat_map(move |x| dot_at(x, y))).collect()
}

fn dot_of(strip: &Pixmap, x: usize, y: usize) -> [u8; BYTES_A_DOT] {
    let at = (y * strip.width() as usize + x) * BYTES_A_DOT;
    [strip.data()[at], strip.data()[at + 1], strip.data()[at + 2], strip.data()[at + 3]]
}

/// A dot of the strip, whose colour is already weighed by how solid it is, with the ground showing through
/// the rest of the way.
fn over_the_ground([red, green, blue, solid]: [u8; BYTES_A_DOT]) -> [u8; BYTES_A_DOT] {
    let see_through = SEEN_THROUGH_FULLY - u16::from(solid);
    let over = |own: u8, ground: u8| (u16::from(own) + u16::from(ground) * see_through / SEEN_THROUGH_FULLY) as u8;
    [over(red, GROUND[0]), over(green, GROUND[1]), over(blue, GROUND[2]), 255]
}

#[cfg(test)]
mod tests {
    use super::*;
    use resvg::tiny_skia::Color;

    const DOTS_ACROSS_A_FRAME: u32 = 2;
    const RED: [u8; BYTES_A_DOT] = [255, 0, 0, 255];

    /// A strip of frames two dots square, all seen through but for one that is red all over.
    fn strip_with_a_red_frame(red_frame: u32) -> Vec<u8> {
        let mut strip = Pixmap::new(FRAMES as u32 * DOTS_ACROSS_A_FRAME, DOTS_ACROSS_A_FRAME).unwrap();
        let red = Color::from_rgba8(RED[0], RED[1], RED[2], RED[3]).premultiply().to_color_u8();
        let first_dot = (red_frame * DOTS_ACROSS_A_FRAME) as usize;
        for row in 0..DOTS_ACROSS_A_FRAME as usize {
            let at = row * strip.width() as usize + first_dot;
            strip.pixels_mut()[at..at + DOTS_ACROSS_A_FRAME as usize].fill(red);
        }
        strip.encode_png().unwrap()
    }

    fn dot(picture: &[u8], x: usize, y: usize) -> [u8; BYTES_A_DOT] {
        let at = (y * SCREEN_WIDTH + x) * BYTES_A_DOT;
        [picture[at], picture[at + 1], picture[at + 2], picture[at + 3]]
    }

    #[test]
    fn each_frame_is_shown_bigger_in_the_middle_of_the_screen_on_the_dark_ground() {
        let pictures = pictures_of(&strip_with_a_red_frame(4)).unwrap();
        let (left, top) = (SCREEN_WIDTH / 2 - 3, SCREEN_HEIGHT / 2 - 3);

        assert_eq!(pictures.len(), FRAMES);
        assert!(pictures.iter().all(|picture| picture.len() == SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_A_DOT));
        assert_eq!((dot(&pictures[4], left, top), dot(&pictures[4], left + 5, top + 5)), (RED, RED), "two dots square, three times bigger");
        assert_eq!((dot(&pictures[4], left - 1, top), dot(&pictures[4], left + 6, top + 5)), (GROUND, GROUND));
        assert_eq!(dot(&pictures[3], left, top), GROUND, "a frame that is seen through shows the ground");
    }

    #[test]
    fn a_dot_that_is_half_solid_lets_half_of_the_ground_through() {
        let half_solid_white = [128, 128, 128, 128];

        assert_eq!(over_the_ground(half_solid_white), [136, 136, 137, 255]);
        assert_eq!(over_the_ground([0, 0, 0, 0]), GROUND);
    }

    #[test]
    fn the_frames_follow_the_clock_round_and_round() {
        assert_eq!(frame_after(Duration::ZERO), 0);
        assert_eq!(frame_after(FRAME_LASTS * 3 + Duration::from_millis(10)), 3);
        assert_eq!(frame_after(FRAME_LASTS * (FRAMES as u32 + 2)), 2);
    }

    #[test]
    fn something_that_is_not_a_picture_is_refused_in_words() {
        assert!(pictures_of(b"not a picture").unwrap_err().starts_with("The blacksmith's picture could not be read"));
    }

    #[test]
    fn while_he_is_shown_every_light_is_dark() {
        let shown = with_every_light_dark(&[7; 8]);

        assert_eq!((shown.pad_colours, shown.lit_buttons.len()), ([[0; 3]; PADS], 0));
    }
}
