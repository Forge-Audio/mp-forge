//! A contact sheet: a few frames of a drawn picture, one above the other on a dark stand-in for the
//! MPC's page, kept small enough to show to the AI so it can see what it drew.

use resvg::tiny_skia::{Color, FilterQuality, IntRect, Paint, Pixmap, PixmapPaint, Rect, Transform};

const MOST_FRAMES_SHOWN: u32 = 4;
/// Neither side of a sheet is longer than this, which keeps the cost of showing one low.
const LONGEST_SIDE: u32 = 768;
const LINE_BETWEEN_FRAMES: u32 = 4;
const PAGE_GREY: u8 = 26;
const LINE_GREY: u8 = 120;

pub struct ContactSheet {
    /// The frames on the sheet from top to bottom, counted from one.
    pub frames_shown: Vec<u32>,
    pub png: Vec<u8>,
}

/// Makes the sheet of a film strip: every frame when there are few, otherwise an even spread from first to last.
pub fn contact_sheet_of(film_strip_png: &[u8], frames: u32) -> Result<ContactSheet, String> {
    let strip = Pixmap::decode_png(film_strip_png).map_err(|error| format!("The drawn picture could not be read: {error}"))?;
    let layout = SheetLayout::of(&strip, frames).ok_or("The drawn picture has no frames to show.")?;
    let mut sheet = layout.sheet_of_lines().ok_or("The drawn picture is too small to show.")?;
    for (place, frame) in (0..).zip(&layout.frames_shown) {
        layout.lay_frame(&mut sheet, &strip, place, *frame);
    }
    let png = sheet.encode_png().map_err(|error| format!("The sheet of frames could not be made: {error}"))?;
    Ok(ContactSheet { frames_shown: layout.frames_shown.iter().map(|frame| frame + 1).collect(), png })
}

/// Where each frame comes from in the strip and where it goes on the sheet.
struct SheetLayout {
    frame_width: u32,
    frame_height: u32,
    /// The frames to show, counted from zero.
    frames_shown: Vec<u32>,
    scale: f32,
}

impl SheetLayout {
    fn of(strip: &Pixmap, frames: u32) -> Option<Self> {
        let frame_height = strip.height().checked_div(frames).filter(|height| *height > 0)?;
        let frames_shown = spread_of(frames);
        let rows = u32::try_from(frames_shown.len()).ok()?;
        let unscaled_height = rows * frame_height + (rows - 1) * LINE_BETWEEN_FRAMES;
        let scale = (LONGEST_SIDE as f32 / strip.width().max(unscaled_height) as f32).min(1.0);
        Some(Self { frame_width: strip.width(), frame_height, frames_shown, scale })
    }

    fn row_width(&self) -> u32 {
        scaled(self.frame_width, self.scale)
    }

    fn row_height(&self) -> u32 {
        scaled(self.frame_height, self.scale)
    }

    fn top_of_row(&self, place: u32) -> u32 {
        place * (self.row_height() + LINE_BETWEEN_FRAMES)
    }

    /// A sheet that is all dividing line. Laying each frame's dark page on it leaves the lines between them.
    fn sheet_of_lines(&self) -> Option<Pixmap> {
        let rows = u32::try_from(self.frames_shown.len()).ok()?;
        let mut sheet = Pixmap::new(self.row_width(), self.top_of_row(rows) - LINE_BETWEEN_FRAMES)?;
        sheet.fill(grey(LINE_GREY));
        Some(sheet)
    }

    fn lay_frame(&self, sheet: &mut Pixmap, strip: &Pixmap, place: u32, frame: u32) {
        let top = self.top_of_row(place) as f32;
        if let Some(row) = Rect::from_xywh(0.0, top, self.row_width() as f32, self.row_height() as f32) {
            sheet.fill_rect(row, &page_paint(), Transform::identity(), None);
        }
        if let Some(cut_out) = self.frame_cut_from(strip, frame) {
            let shrunk_into_row = Transform::from_scale(self.scale, self.scale).post_translate(0.0, top);
            sheet.draw_pixmap(0, 0, cut_out.as_ref(), &smoothly(), shrunk_into_row, None);
        }
    }

    fn frame_cut_from(&self, strip: &Pixmap, frame: u32) -> Option<Pixmap> {
        let top_in_strip = i32::try_from(frame * self.frame_height).ok()?;
        strip.as_ref().clone_rect(IntRect::from_xywh(0, top_in_strip, self.frame_width, self.frame_height)?)
    }
}

/// An even spread of frames that always has the first and the last.
fn spread_of(frames: u32) -> Vec<u32> {
    let shown = frames.min(MOST_FRAMES_SHOWN);
    if shown < 2 {
        return vec![0];
    }
    (0..shown).map(|place| place * (frames - 1) / (shown - 1)).collect()
}

fn scaled(length: u32, scale: f32) -> u32 {
    ((length as f32 * scale).round() as u32).max(1)
}

fn grey(level: u8) -> Color {
    Color::from_rgba8(level, level, level, 255)
}

fn page_paint() -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(grey(PAGE_GREY));
    paint
}

fn smoothly() -> PixmapPaint {
    PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::film_strip::test_support::{pixel_of, size_of};
    use crate::film_strip::{FilmStrip, FrameSize, Timing};

    const RED: [u8; 4] = [255, 0, 0, 255];
    const PAGE: [u8; 4] = [PAGE_GREY, PAGE_GREY, PAGE_GREY, 255];
    const LINE: [u8; 4] = [LINE_GREY, LINE_GREY, LINE_GREY, 255];
    /// A red bar that starts at the left edge and slides right by its own width over the movement.
    const SLIDING_BAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><rect x="{10 * t}" y="0" width="10" height="10" fill="red"/></svg>"#;

    fn strip_of_sliding_bar(frame_size: FrameSize, frames: u32) -> Vec<u8> {
        FilmStrip { drawing: SLIDING_BAR, frame_size, frames, timing: Timing::EndToEnd }.as_png().unwrap()
    }

    #[test]
    fn a_short_strip_is_shown_whole_on_a_dark_page_with_a_line_between_frames() {
        let strip = strip_of_sliding_bar(FrameSize { width: 20, height: 10 }, 2);

        let sheet = contact_sheet_of(&strip, 2).unwrap();

        assert_eq!(sheet.frames_shown, [1, 2]);
        assert_eq!(size_of(&sheet.png), (20, 10 + LINE_BETWEEN_FRAMES + 10));
        assert_eq!(pixel_of(&sheet.png, 2, 5), RED);
        assert_eq!(pixel_of(&sheet.png, 17, 5), PAGE);
        assert_eq!(pixel_of(&sheet.png, 10, 11), LINE);
        assert_eq!(pixel_of(&sheet.png, 17, 10 + LINE_BETWEEN_FRAMES + 5), RED);
    }

    #[test]
    fn a_long_strip_shows_an_even_spread_from_its_first_frame_to_its_last() {
        let strip = strip_of_sliding_bar(FrameSize { width: 20, height: 10 }, 12);

        let sheet = contact_sheet_of(&strip, 12).unwrap();

        assert_eq!(sheet.frames_shown, [1, 4, 8, 12]);
        assert_eq!(spread_of(1), [0]);
        assert_eq!(spread_of(5), [0, 1, 2, 4]);
    }

    #[test]
    fn a_big_picture_is_shrunk_so_neither_side_is_too_long() {
        let strip = strip_of_sliding_bar(FrameSize { width: 1280, height: 628 }, 3);

        let sheet = contact_sheet_of(&strip, 3).unwrap();

        let (width, height) = size_of(&sheet.png);
        assert!(width <= LONGEST_SIDE && height <= LONGEST_SIDE + 2 * LINE_BETWEEN_FRAMES, "{width} x {height}");
        assert_eq!(pixel_of(&sheet.png, width / 8, height / 6), RED);
    }

    #[test]
    fn something_that_is_not_a_drawn_picture_is_refused() {
        assert!(contact_sheet_of(b"not a png", 3).is_err());
        assert!(contact_sheet_of(&strip_of_sliding_bar(FrameSize { width: 20, height: 10 }, 2), 0).is_err());
    }
}
