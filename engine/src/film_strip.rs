//! Turns one drawing into a film strip: every frame of a picture, stacked top to bottom in a single PNG.
//!
//! This is the image format the MPC plays. The drawing is SVG, and the sums in it are worked out for
//! each frame. Nothing outside the drawing is ever loaded: no other images, no fonts, no files.

use std::time::{Duration, Instant};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{ImageHrefResolver, Options, Tree};

use crate::frame_expression::{Moment, fill_in};

const BYTES_PER_PIXEL: usize = 4;
const LONGEST_DRAWING_TIME: Duration = Duration::from_secs(20);

/// Parts of SVG that would not be drawn, with the reason given to whoever wrote the drawing.
const PARTS_NOT_DRAWN: [(&str, &str); 5] = [
    ("<image", "Other images cannot be brought into a drawing. Draw with shapes."),
    ("<text", "Lettering is not drawn, because no fonts are loaded. Draw letters as shapes, or use a text heading in ui.json."),
    ("<foreignobject", "Only SVG shapes are drawn."),
    ("<script", "A drawing cannot hold a script. Use the sums in curly brackets to move things."),
    ("<!doctype", "A drawing cannot hold a document type or entities."),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSize {
    pub width: u32,
    pub height: u32,
}

impl FrameSize {
    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// How `t` runs across the frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// The last frame leads back into the first, so `t` stops one step short of 1.
    Looping,
    /// The first frame is one end and the last frame the other, so `t` runs from 0 to exactly 1.
    EndToEnd,
}

pub struct FilmStrip<'a> {
    pub drawing: &'a str,
    pub frame_size: FrameSize,
    pub frames: u32,
    pub timing: Timing,
}

impl FilmStrip<'_> {
    pub fn as_png(&self) -> Result<Vec<u8>, String> {
        refuse_parts_not_drawn(self.drawing)?;
        let mut strip = self.blank_strip()?;
        let started = Instant::now();
        for frame in 0..self.frames {
            if started.elapsed() > LONGEST_DRAWING_TIME {
                return Err("The drawing takes too long to draw. Make it simpler or use fewer frames.".to_string());
            }
            let drawn = self.frame_drawn_at(self.moment_of(frame))?;
            self.rows_of(frame, strip.data_mut()).copy_from_slice(drawn.data());
        }
        strip.encode_png().map_err(|error| format!("The picture could not be saved: {error}"))
    }

    fn blank_strip(&self) -> Result<Pixmap, String> {
        let strip = Pixmap::new(self.frame_size.width, self.frame_size.height * self.frames);
        strip.ok_or_else(|| "A picture needs a size and at least one frame.".to_string())
    }

    fn moment_of(&self, frame: u32) -> Moment {
        let steps = match self.timing {
            Timing::Looping => self.frames,
            Timing::EndToEnd => self.frames.saturating_sub(1).max(1),
        };
        Moment { frame, frames: self.frames, progress: f64::from(frame) / f64::from(steps) }
    }

    fn frame_drawn_at(&self, moment: Moment) -> Result<Pixmap, String> {
        let explain = |problem: String| format!("Frame {} could not be drawn: {problem}", moment.frame + 1);
        let tree = parsed(&fill_in(self.drawing, &moment).map_err(explain)?).map_err(explain)?;
        let mut frame = Pixmap::new(self.frame_size.width, self.frame_size.height).ok_or("A picture needs a size.")?;
        resvg::render(&tree, self.fitted(&tree), &mut frame.as_mut());
        Ok(frame)
    }

    /// Scales the drawing to fill the frame as far as it can without stretching, and centres it.
    fn fitted(&self, tree: &Tree) -> Transform {
        let (frame_width, frame_height) = (self.frame_size.width as f32, self.frame_size.height as f32);
        let drawing = tree.size();
        let scale = (frame_width / drawing.width()).min(frame_height / drawing.height());
        let left = (frame_width - drawing.width() * scale) / 2.0;
        let top = (frame_height - drawing.height() * scale) / 2.0;
        Transform::from_scale(scale, scale).post_translate(left, top)
    }

    fn rows_of<'s>(&self, frame: u32, strip: &'s mut [u8]) -> &'s mut [u8] {
        let frame_bytes = self.frame_size.pixels() as usize * BYTES_PER_PIXEL;
        let start = frame as usize * frame_bytes;
        &mut strip[start..start + frame_bytes]
    }
}

fn refuse_parts_not_drawn(drawing: &str) -> Result<(), String> {
    let lowercase = drawing.to_ascii_lowercase();
    match PARTS_NOT_DRAWN.iter().find(|(part, _)| lowercase.contains(part)) {
        Some((_, reason)) => Err((*reason).to_string()),
        None => Ok(()),
    }
}

fn parsed(svg: &str) -> Result<Tree, String> {
    Tree::from_str(svg, &options_that_load_nothing()).map_err(|error| format!("it is not SVG that can be read ({error})"))
}

fn options_that_load_nothing() -> Options<'static> {
    let no_images = ImageHrefResolver { resolve_data: Box::new(|_, _, _| None), resolve_string: Box::new(|_, _| None) };
    Options { resources_dir: None, image_href_resolver: no_images, ..Options::default() }
}

#[cfg(test)]
pub mod test_support {
    use resvg::tiny_skia::Pixmap;

    /// The red, green, blue and alpha of one pixel of a PNG.
    pub fn pixel_of(png: &[u8], x: u32, y: u32) -> [u8; 4] {
        let picture = Pixmap::decode_png(png).unwrap();
        let pixel = picture.pixel(x, y).unwrap();
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
    }

    pub fn size_of(png: &[u8]) -> (u32, u32) {
        let picture = Pixmap::decode_png(png).unwrap();
        (picture.width(), picture.height())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{pixel_of, size_of};
    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    /// A red bar that starts at the left edge and slides right by its own width over the movement.
    const SLIDING_BAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><rect x="{10 * t}" y="0" width="10" height="10" fill="red"/></svg>"#;

    fn strip_of(drawing: &str, frames: u32, timing: Timing) -> FilmStrip<'_> {
        FilmStrip { drawing, frame_size: FrameSize { width: 20, height: 10 }, frames, timing }
    }

    #[test]
    fn frames_are_stacked_top_to_bottom_and_each_shows_its_own_moment() {
        let png = strip_of(SLIDING_BAR, 2, Timing::EndToEnd).as_png().unwrap();

        assert_eq!(size_of(&png), (20, 20));
        assert_eq!(pixel_of(&png, 2, 5), RED);
        assert_eq!(pixel_of(&png, 17, 5), CLEAR);
        assert_eq!(pixel_of(&png, 2, 15), CLEAR);
        assert_eq!(pixel_of(&png, 17, 15), RED);
    }

    #[test]
    fn a_looping_strip_stops_one_step_short_so_it_joins_up() {
        assert_eq!(strip_of(SLIDING_BAR, 2, Timing::Looping).moment_of(1).progress, 0.5);
        assert_eq!(strip_of(SLIDING_BAR, 2, Timing::EndToEnd).moment_of(1).progress, 1.0);
        assert_eq!(strip_of(SLIDING_BAR, 1, Timing::EndToEnd).moment_of(0).progress, 0.0);
    }

    #[test]
    fn a_drawing_of_another_shape_is_centred_without_stretching() {
        let square = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"#;

        let png = strip_of(square, 1, Timing::EndToEnd).as_png().unwrap();

        assert_eq!(pixel_of(&png, 10, 5), RED);
        assert_eq!(pixel_of(&png, 2, 5), CLEAR);
        assert_eq!(pixel_of(&png, 17, 5), CLEAR);
    }

    #[test]
    fn nothing_outside_the_drawing_is_ever_loaded() {
        let reaches_out = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><image href="/etc/hosts" width="20" height="10"/></svg>"#;

        let refusal = strip_of(reaches_out, 1, Timing::EndToEnd).as_png().unwrap_err();

        assert!(refusal.contains("Other images cannot be brought into a drawing"), "{refusal}");
    }

    #[test]
    fn a_drawing_that_is_not_svg_names_the_frame_that_failed() {
        let refusal = strip_of("<svg", 3, Timing::Looping).as_png().unwrap_err();

        assert!(refusal.starts_with("Frame 1 could not be drawn"), "{refusal}");
    }
}
