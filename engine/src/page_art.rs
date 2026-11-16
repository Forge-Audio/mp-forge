//! Draws the artwork the musician laid behind a page's controls.
//!
//! All the layers of a page are drawn into one picture the size of the MPC's page, clear wherever there
//! is no artwork. That one picture is what the MPC shows: it never needs the fonts or the separate pictures.
//! A layer can also be drawn on its own, for the editor to show while it is being arranged.

use std::path::Path;

use resvg::tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};
use resvg::usvg::Tree;

use crate::art_shelf::ArtShelf;
use crate::fonts::{has_font, options_with_this_computers_fonts};
use crate::layers::{Art, Block, Frame, Layer, PlacedArt, Shape, Words};
use crate::screen_layout::WHOLE_PAGE;

/// A line of words is this much taller than its letters, which leaves room for tails below the line.
const LINE_HEIGHT: f32 = 1.25;
/// How far below the top of its line the letters stand.
const BASELINE: f32 = 0.95;
const BOLD_WEIGHT: u32 = 700;
const REGULAR_WEIGHT: u32 = 400;

/// The pixels a layer covers on the page. Words are measured in their own font.
pub fn frame_of(layer: &Layer) -> Frame {
    match &layer.art {
        Art::Block(block) => block.frame,
        Art::Picture(picture) => picture.frame,
        Art::Words(words) => Frame { x: words.x, y: words.y, width: width_of(words), height: line_height_of(words) },
    }
}

/// Why this artwork cannot be drawn, in words for the musician. Empty when it all can.
pub fn problems_with(plugin: &Path, page_name: &str, layers: &[Layer]) -> Vec<String> {
    let shelf = ArtShelf::of_plugin(plugin);
    let about = |layer: &Layer, problem: String| format!("On page `{page_name}`, {problem} Change or delete layer {} in EDIT THE LOOK.", layer.id);
    layers.iter().filter_map(|layer| problem_with(&shelf, layer).map(|problem| about(layer, problem))).collect()
}

fn problem_with(shelf: &ArtShelf, layer: &Layer) -> Option<String> {
    match &layer.art {
        Art::Words(words) if !has_font(&words.font) => Some(format!("the words \"{}\" use the font {}, which this computer does not have.", words.text, words.font)),
        Art::Picture(picture) if shelf.decoded(&picture.file).is_none() => Some(format!("a picture ({}) is no longer in the plugin.", picture.file)),
        _ => None,
    }
}

/// Every layer that is not hidden, drawn into one picture the size of the MPC's page.
pub fn flattened(plugin: &Path, layers: &[Layer]) -> Result<Vec<u8>, String> {
    let mut page = Pixmap::new(WHOLE_PAGE.width, WHOLE_PAGE.height).ok_or("The page could not be drawn.")?;
    let easel = Easel { shelf: ArtShelf::of_plugin(plugin), left: 0, top: 0 };
    for layer in layers.iter().filter(|layer| !layer.hidden) {
        easel.draw(layer, &mut page)?;
    }
    saved(&page)
}

/// One layer drawn by itself and upright, as a picture exactly the size of the pixels it covers before it is tilted.
/// Whoever shows the picture tilts it.
pub fn drawn_alone(plugin: &Path, layer: &Layer) -> Result<Vec<u8>, String> {
    let frame = frame_of(layer);
    let upright = Layer { tilt: 0, ..layer.clone() };
    let mut picture = Pixmap::new(frame.width, frame.height).ok_or("A layer needs a size.")?;
    Easel { shelf: ArtShelf::of_plugin(plugin), left: frame.x, top: frame.y }.draw(&upright, &mut picture)?;
    saved(&picture)
}

/// Whether any of the layers would show on the page.
pub fn shows_anything(layers: &[Layer]) -> bool {
    layers.iter().any(|layer| !layer.hidden)
}

fn saved(picture: &Pixmap) -> Result<Vec<u8>, String> {
    picture.encode_png().map_err(|error| format!("The artwork could not be saved: {error}"))
}

/// Draws layers onto a canvas whose top left corner sits at (`left`, `top`) of the page.
struct Easel {
    shelf: ArtShelf,
    left: i32,
    top: i32,
}

impl Easel {
    fn draw(&self, layer: &Layer, canvas: &mut Pixmap) -> Result<(), String> {
        match &layer.art {
            Art::Block(block) => self.draw_shapes(&block_shapes(block), layer, canvas),
            Art::Words(words) => self.draw_shapes(&lettering(words), layer, canvas),
            Art::Picture(picture) => self.draw_picture(picture, layer, canvas),
        }
    }

    fn draw_shapes(&self, shapes: &str, layer: &Layer, canvas: &mut Pixmap) -> Result<(), String> {
        let drawing = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}"><g transform="translate({} {})" opacity="{}">{shapes}</g></svg>"#,
            canvas.width(),
            canvas.height(),
            -self.left,
            -self.top,
            share_of(layer.opacity),
        );
        let tree = Tree::from_str(&drawing, &options_with_this_computers_fonts()).map_err(|error| format!("Layer {} could not be drawn: {error}", layer.id))?;
        resvg::render(&tree, self.tilt_of(layer), &mut canvas.as_mut());
        Ok(())
    }

    /// The turn that tilts a layer about the middle of the pixels it covers, as measured on this canvas.
    fn tilt_of(&self, layer: &Layer) -> Transform {
        let frame = frame_of(layer);
        let middle_across = (frame.x - self.left) as f32 + frame.width as f32 / 2.0;
        let middle_down = (frame.y - self.top) as f32 + frame.height as f32 / 2.0;
        Transform::from_rotate_at(layer.tilt as f32, middle_across, middle_down)
    }

    fn draw_picture(&self, placed: &PlacedArt, layer: &Layer, canvas: &mut Pixmap) -> Result<(), String> {
        let whole = self.shelf.decoded(&placed.file).ok_or_else(|| format!("Layer {}: its picture, {}, is no longer in the plugin.", layer.id, placed.file))?;
        let shown = part_shown(&whole, placed).unwrap_or(whole);
        let stretch = Transform::from_scale(placed.frame.width as f32 / shown.width() as f32, placed.frame.height as f32 / shown.height() as f32);
        let onto_canvas = stretch.post_translate((placed.frame.x - self.left) as f32, (placed.frame.y - self.top) as f32).post_concat(self.tilt_of(layer));
        let paint = PixmapPaint { opacity: share_of(layer.opacity), quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
        canvas.draw_pixmap(0, 0, shown.as_ref(), &paint, onto_canvas, None);
        Ok(())
    }
}

/// The part of a picture its crop asks for. A crop that reaches outside the picture is cut back to it.
fn part_shown(whole: &Pixmap, placed: &PlacedArt) -> Option<Pixmap> {
    let crop = placed.crop?;
    let width = crop.width.min(whole.width().saturating_sub(crop.x));
    let height = crop.height.min(whole.height().saturating_sub(crop.y));
    let inside = IntRect::from_xywh(i32::try_from(crop.x).ok()?, i32::try_from(crop.y).ok()?, width, height)?;
    whole.as_ref().clone_rect(inside)
}

fn share_of(opacity: u32) -> f32 {
    opacity.min(100) as f32 / 100.0
}

// --- A block of colour -------------------------------------------------------------------------

fn block_shapes(block: &Block) -> String {
    let fill = match &block.fade {
        Some(_) => "url(#fade)".to_string(),
        None => block.colour.written(),
    };
    let filled = outline_of(block, 0.0, &format!(r#"fill="{fill}""#));
    let edged = block.edge.map(|edge| {
        outline_of(block, edge.width as f32 / 2.0, &format!(r#"fill="none" stroke="{}" stroke-width="{}""#, edge.colour.written(), edge.width))
    });
    format!("{}{filled}{}", fade_of(block), edged.unwrap_or_default())
}

/// The block's outline, drawn `inset` pixels inside its frame so that an edge line never spills out of it.
fn outline_of(block: &Block, inset: f32, paint: &str) -> String {
    let frame = block.frame;
    let (width, height) = ((frame.width as f32 - inset * 2.0).max(0.0), (frame.height as f32 - inset * 2.0).max(0.0));
    let (left, top) = (frame.x as f32 + inset, frame.y as f32 + inset);
    match block.shape {
        Shape::Ellipse => format!(r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {paint}/>"#, left + width / 2.0, top + height / 2.0, width / 2.0, height / 2.0),
        Shape::Rectangle => {
            let corner = (block.corner as f32 - inset).max(0.0);
            format!(r#"<rect x="{left}" y="{top}" width="{width}" height="{height}" rx="{corner}" {paint}/>"#)
        }
    }
}

/// The fade, from the block's own colour to the fade's, between two points measured across the block.
fn fade_of(block: &Block) -> String {
    let Some(fade) = &block.fade else { return String::new() };
    let frame = block.frame;
    let across = |hundredths: i32| frame.x as f32 + frame.width as f32 * hundredths as f32 / 100.0;
    let down = |hundredths: i32| frame.y as f32 + frame.height as f32 * hundredths as f32 / 100.0;
    format!(
        r#"<defs><linearGradient id="fade" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs>"#,
        across(fade.from.x),
        down(fade.from.y),
        across(fade.to.x),
        down(fade.to.y),
        block.colour.written(),
        fade.colour.written(),
    )
}

// --- Words -------------------------------------------------------------------------------------

/// The words as a drawing. They are written as plain text, so nothing in them can be read as markup.
fn lettering(words: &Words) -> String {
    format!(
        r#"<text x="{}" y="{}" font-family="'{}'" font-size="{}" font-weight="{}" fill="{}" xml:space="preserve">{}</text>"#,
        words.x,
        words.y as f32 + words.size as f32 * BASELINE,
        words.font,
        words.size,
        if words.bold { BOLD_WEIGHT } else { REGULAR_WEIGHT },
        words.colour.written(),
        as_plain_text(&words.text),
    )
}

fn as_plain_text(words: &str) -> String {
    words.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn line_height_of(words: &Words) -> u32 {
    (words.size as f32 * LINE_HEIGHT).ceil() as u32
}

/// How wide the words come out in their font. Words whose font is missing still get a width, so they can be picked up.
fn width_of(words: &Words) -> u32 {
    let at_the_corner = Words { x: 0, y: 0, ..words.clone() };
    let drawing = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{}</svg>"#, lettering(&at_the_corner));
    let measured = Tree::from_str(&drawing, &options_with_this_computers_fonts()).ok().and_then(|tree| tree.root().abs_bounding_box());
    let unmeasured = words.size * words.text.chars().count() as u32 / 2;
    measured.map(|inked| inked.right().ceil() as u32).filter(|width| *width > 0).unwrap_or(unmeasured).clamp(1, crate::layers::LONGEST_SIDE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachments::test_support::png_of;
    use crate::film_strip::test_support::{pixel_of, size_of};
    use crate::fonts::test_support::any_font;
    use crate::layers::test_support::{BLUE, RED, block_over, red_block, solid};
    use crate::layers::{Edge, Fade, Spot};
    use crate::screen_layout::Area;
    use crate::workspace::test_support::TemporaryFolder;

    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    const SOLID_RED: [u8; 4] = [255, 0, 0, 255];
    const SOLID_BLUE: [u8; 4] = [0, 0, 255, 255];
    const GREY: [u8; 4] = [90, 90, 90, 255];

    fn frame(x: i32, y: i32, width: u32, height: u32) -> Frame {
        Frame { x, y, width, height }
    }

    fn flattened_alone(layers: &[Layer]) -> Vec<u8> {
        flattened(&TemporaryFolder::create("page-art").0, layers).unwrap()
    }

    fn words_saying(text: &str, font: &str) -> Words {
        Words { text: text.to_string(), font: font.to_string(), size: 40, bold: false, colour: RED, x: 100, y: 200 }
    }

    #[test]
    fn a_page_of_artwork_is_the_size_of_the_mpcs_page_and_clear_where_nothing_is() {
        let page = flattened_alone(&[red_block(1, frame(100, 100, 200, 50))]);

        assert_eq!(size_of(&page), (1280, 628));
        assert_eq!(pixel_of(&page, 150, 120), SOLID_RED);
        assert_eq!(pixel_of(&page, 50, 120), CLEAR);
        assert_eq!(pixel_of(&page, 150, 160), CLEAR);
    }

    #[test]
    fn later_layers_cover_earlier_ones_and_hidden_ones_are_left_out() {
        let blue_over = solid(2, Art::Block(block_over(frame(150, 100, 200, 50), BLUE)));
        let hidden = Layer { hidden: true, ..red_block(3, frame(0, 0, 1280, 628)) };

        let page = flattened_alone(&[red_block(1, frame(100, 100, 200, 50)), blue_over, hidden.clone()]);

        assert_eq!(pixel_of(&page, 120, 120), SOLID_RED);
        assert_eq!(pixel_of(&page, 200, 120), SOLID_BLUE);
        assert_eq!(pixel_of(&page, 600, 400), CLEAR);
        assert!(shows_anything(&[red_block(1, frame(0, 0, 5, 5))]) && !shows_anything(&[hidden]));
    }

    #[test]
    fn a_tilted_layer_is_turned_clockwise_about_its_own_middle() {
        let lying_flat = frame(100, 100, 100, 20);
        let quarter_turn = Layer { tilt: 90, ..red_block(1, lying_flat) };
        let leaning = Layer { tilt: 45, ..red_block(1, lying_flat) };

        let stood_up = flattened_alone(&[quarter_turn]);
        let leant = flattened_alone(&[leaning]);

        assert_eq!(pixel_of(&stood_up, 150, 70), SOLID_RED, "its end now reaches above where it lay");
        assert_eq!(pixel_of(&stood_up, 110, 110), CLEAR, "and it no longer reaches sideways");
        assert_eq!(pixel_of(&leant, 180, 140), SOLID_RED, "its right end goes down");
        assert_eq!(pixel_of(&leant, 180, 80), CLEAR);
    }

    #[test]
    fn a_layer_drawn_alone_stays_upright_for_whoever_shows_it_to_tilt() {
        let plugin = TemporaryFolder::create("page-art-upright");
        let tilted = Layer { tilt: 30, ..red_block(1, frame(10, 10, 40, 20)) };
        let upright = red_block(1, frame(10, 10, 40, 20));

        assert_eq!(drawn_alone(&plugin.0, &tilted).unwrap(), drawn_alone(&plugin.0, &upright).unwrap());
    }

    #[test]
    fn a_layer_that_is_partly_see_through_lets_the_page_show() {
        let faint = Layer { opacity: 50, ..red_block(1, frame(0, 0, 100, 100)) };

        let [red, green, _, alpha] = pixel_of(&flattened_alone(&[faint]), 50, 50);

        assert!((126..=129).contains(&alpha) && red == alpha && green == 0, "{red} {green} {alpha}");
    }

    #[test]
    fn an_ellipse_and_rounded_corners_leave_the_corners_of_their_frame_clear() {
        let ellipse = solid(1, Art::Block(Block { shape: Shape::Ellipse, ..block_over(frame(0, 0, 200, 100), RED) }));
        let rounded = solid(2, Art::Block(Block { corner: 40, ..block_over(frame(300, 0, 200, 100), RED) }));

        let page = flattened_alone(&[ellipse, rounded]);

        assert_eq!((pixel_of(&page, 100, 50), pixel_of(&page, 3, 3)), (SOLID_RED, CLEAR));
        assert_eq!((pixel_of(&page, 400, 50), pixel_of(&page, 302, 2), pixel_of(&page, 400, 2)), (SOLID_RED, CLEAR, SOLID_RED));
    }

    #[test]
    fn a_fade_runs_from_the_blocks_colour_to_the_fades_between_its_two_ends() {
        let fade = Fade { colour: BLUE, from: Spot { x: 0, y: 0 }, to: Spot { x: 100, y: 0 } };
        let fading = solid(1, Art::Block(Block { fade: Some(fade), ..block_over(frame(100, 0, 400, 100), RED) }));

        let page = flattened_alone(&[fading]);

        let [red_at_start, _, blue_at_start, _] = pixel_of(&page, 102, 50);
        let [red_at_end, _, blue_at_end, _] = pixel_of(&page, 497, 50);
        let [red_halfway, _, blue_halfway, _] = pixel_of(&page, 300, 50);
        assert!(red_at_start > 245 && blue_at_start < 10 && red_at_end < 10 && blue_at_end > 245);
        assert!((110..=145).contains(&red_halfway) && (110..=145).contains(&blue_halfway), "{red_halfway} {blue_halfway}");
    }

    #[test]
    fn an_edge_is_drawn_just_inside_the_block() {
        let edged = solid(1, Art::Block(Block { edge: Some(Edge { colour: BLUE, width: 10 }), ..block_over(frame(100, 100, 200, 100), RED) }));

        let page = flattened_alone(&[edged]);

        assert_eq!((pixel_of(&page, 104, 150), pixel_of(&page, 200, 150), pixel_of(&page, 97, 150)), (SOLID_BLUE, SOLID_RED, CLEAR));
    }

    #[test]
    fn a_picture_is_stretched_over_its_frame_and_can_show_only_part_of_itself() {
        let plugin = TemporaryFolder::create("page-art-picture");
        let file = ArtShelf::of_plugin(&plugin.0).add(&png_of(40, 20)).unwrap();
        let stretched = PlacedArt { file: file.clone(), frame: frame(100, 100, 400, 300), crop: None };
        let cropped = PlacedArt { file, frame: frame(800, 100, 100, 100), crop: Some(Area { x: 10, y: 5, width: 10, height: 10 }) };

        let page = flattened(&plugin.0, &[solid(1, Art::Picture(stretched)), solid(2, Art::Picture(cropped))]).unwrap();

        assert_eq!((pixel_of(&page, 110, 110), pixel_of(&page, 490, 390), pixel_of(&page, 510, 390)), (GREY, GREY, CLEAR));
        assert_eq!((pixel_of(&page, 850, 150), pixel_of(&page, 790, 150)), (GREY, CLEAR));
    }

    #[test]
    fn a_picture_that_has_gone_is_a_problem_said_in_plain_words() {
        let plugin = TemporaryFolder::create("page-art-missing");
        let missing = solid(4, Art::Picture(PlacedArt { file: "art-9.png".to_string(), frame: frame(0, 0, 10, 10), crop: None }));

        let problems = problems_with(&plugin.0, "MAIN", &[red_block(1, frame(0, 0, 5, 5)), missing.clone()]);

        assert_eq!(problems, ["On page `MAIN`, a picture (art-9.png) is no longer in the plugin. Change or delete layer 4 in EDIT THE LOOK."]);
        assert!(flattened(&plugin.0, &[missing]).unwrap_err().contains("art-9.png"));
    }

    #[test]
    fn words_are_drawn_in_their_font_inside_the_frame_measured_for_them() {
        let Some(font) = any_font() else { return };
        let words = solid(1, Art::Words(words_saying("MMMM", &font)));

        let measured = frame_of(&words);
        let page = flattened_alone(&[words.clone()]);

        let picture = Pixmap::decode_png(&page).unwrap();
        let inked = |x: u32, y: u32| picture.pixel(x, y).is_some_and(|pixel| pixel.alpha() > 0);
        let inside = (measured.x as u32..measured.x as u32 + measured.width).any(|x| (measured.y as u32..measured.y as u32 + measured.height).any(|y| inked(x, y)));
        let outside = (0..1280).any(|x| (0..628).any(|y| inked(x, y) && !(100..100 + measured.width + 2).contains(&x)));
        assert_eq!((measured.x, measured.y, measured.height), (100, 200, 50));
        assert!(measured.width > 40 && inside && !outside, "{measured:?}");
        assert_eq!(size_of(&drawn_alone(&TemporaryFolder::create("page-art-alone").0, &words).unwrap()), (measured.width, 50));
    }

    #[test]
    fn words_in_a_font_this_computer_lacks_are_a_problem_but_can_still_be_picked_up() {
        let plugin = TemporaryFolder::create("page-art-font");
        let words = solid(2, Art::Words(words_saying("TAPE", "No Such Font 12345")));

        let problems = problems_with(&plugin.0, "MAIN", &[words.clone()]);

        assert!(problems[0].contains("use the font No Such Font 12345, which this computer does not have"), "{problems:?}");
        assert!(frame_of(&words).width >= 1);
    }

    #[test]
    fn words_can_never_be_read_as_markup() {
        let drawing = lettering(&words_saying("<b> & more", "Arial"));

        assert!(drawing.contains(">&lt;b&gt; &amp; more</text>"), "{drawing}");
    }

    #[test]
    fn a_layer_drawn_alone_fills_a_picture_of_its_own_size() {
        let plugin = TemporaryFolder::create("page-art-one");

        let alone = drawn_alone(&plugin.0, &red_block(1, frame(-50, 300, 120, 40))).unwrap();

        assert_eq!(size_of(&alone), (120, 40));
        assert_eq!((pixel_of(&alone, 0, 0), pixel_of(&alone, 119, 39)), (SOLID_RED, SOLID_RED));
    }
}
