//! Draws each page of a plugin's screen the way the MPC lays it out, to show to the AI.
//!
//! Controls sit where Akai's generator puts them (the measurements are its own, from `gen_ui.py`), or
//! where the musician moved them by hand, with the plugin's own built artwork and pictures, and every
//! control at its starting setting. Lettering is in whatever plain font this computer has, so its exact
//! shapes differ from the MPC's.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use resvg::tiny_skia::{Color, FilterQuality, IntRect, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};
use resvg::usvg::Tree;
use serde_json::Value;

use crate::arranged_screen::{ArrangedItem, ArrangedPage, ArrangedPicture, arranged_screen_of};
use crate::artwork::{drawn_file_of, drawn_pictures, faded, page_art_image};
use crate::control_parts::{self, LabelLook, Parts};
use crate::fonts::options_with_this_computers_fonts;
use crate::library;
use crate::look::{Depth, FULLY_SOLID};
use crate::pictures::{Motion, Picture};
use crate::screen_layout::{Area, WHOLE_PAGE, area_of_slot};
use crate::sdk::built_skin_image;

/// A page is shown this wide: large enough to read its lettering, small enough to be cheap to show.
const VIEW_WIDTH: u32 = 1024;
/// A close-up is enlarged by whole pixels, as far as fits a picture of this size.
const CLOSE_UP_FITS: (u32, u32) = (1200, 900);
const MOST_ENLARGED: u32 = 6;
const SMALLEST_CLOSE_UP: u32 = 16;
const SLOTS_ON_A_PAGE: usize = 16;
/// Every moving image in Akai's control kit has this many frames.
const KIT_FRAMES: u32 = 127;
/// The MPC gives a letter's full height. The letters themselves come out at about two thirds of it.
const LETTER_SIZE_FOR_HEIGHT: f32 = 0.66;
const FONTS: &str = "Titillium Web, Helvetica Neue, Helvetica, Arial, sans-serif";
const TRACK_GREY: &str = "ff434347";
const DEFAULT_BACKGROUND: &str = "ff1e1d1e";
const DEFAULT_TEXT: &str = "ffffffff";
const DEFAULT_MUTED: &str = "ff6a6a6c";
const DEFAULT_KIT: &str = "mpc-red";
/// The colours of the control kit's six themes, from the SDK's `make_ui_kit.py`.
const KIT_COLOURS: [(&str, &str); 6] =
    [("mpc-red", "ffd31145"), ("amber", "ffffa21a"), ("cyan", "ff19c6e0"), ("lime", "ff8fd21f"), ("violet", "ff9b63ff"), ("mono", "ffe4e4e6")];

const fn area(x: u32, y: u32, width: u32, height: u32) -> Area {
    Area { x, y, width, height }
}

/// Where the parts of each kind of control sit inside its 278 by 107 slot.
mod places {
    use super::{Area, area};

    pub const KIT_KNOB: Area = area(8, 8, 90, 90);
    pub const KIT_KNOB_NAME: Area = area(110, 12, 156, 36);
    pub const KIT_KNOB_VALUE: Area = area(110, 52, 156, 42);
    pub const PLAIN_KNOB: Area = area(15, 15, 77, 77);
    pub const PLAIN_KNOB_NAME: Area = area(104, 14, 160, 38);
    pub const PLAIN_KNOB_VALUE: Area = area(104, 54, 160, 40);
    pub const FADER_NAME: Area = area(15, 8, 135, 34);
    pub const FADER_VALUE: Area = area(150, 8, 113, 34);
    pub const FADER: Area = area(15, 54, 248, 36);
    pub const TOGGLE_NAME: Area = area(15, 12, 170, 36);
    pub const TOGGLE_VALUE: Area = area(15, 52, 170, 42);
    pub const TOGGLE: Area = area(186, 20, 84, 68);
    pub const CHOICE_NAME: Area = area(15, 8, 248, 34);
    pub const CHOICE_TOP: u32 = 54;
    pub const CHOICE_HEIGHT: u32 = 40;
    pub const SLIDER_NAME: Area = area(15, 15, 150, 34);
    pub const SLIDER_VALUE: Area = area(165, 15, 98, 34);
    pub const SLIDER_BAR: Area = area(15, 58, 248, 22);

    /// The width of each button and where each starts, for a choice of two, three or four.
    pub fn choice_buttons(choices: usize) -> Option<(u32, &'static [u32])> {
        match choices {
            2 => Some((122, &[15, 141])),
            3 => Some((80, &[15, 99, 183])),
            4 => Some((58, &[15, 77, 139, 201])),
            _ => None,
        }
    }
}

pub struct PageView {
    pub page_name: String,
    pub png: Vec<u8>,
}

/// One part of a page at its real size, enlarged so that small things can be judged.
pub struct CloseUp {
    pub png: Vec<u8>,
    /// The part of the page that is shown, which is the part asked for kept inside the page.
    pub part: Area,
    /// How many times each of the page's pixels is drawn across and down.
    pub times: u32,
}

/// One view for each page of the plugin's screen. A plugin with no layout has none. Before its first build
/// its controls are drawn at a stand-in setting.
pub fn page_views(plugin: &Path) -> Vec<PageView> {
    let views = |screen: &Screen, pages: &[Value]| pages.iter().enumerate().filter_map(|(position, page)| screen.view_of(position, page)).collect();
    on_the_screen_of(plugin, views).unwrap_or_default()
}

/// A close-up of one part of the page with this name. None when the plugin has no such page.
pub fn close_up(plugin: &Path, page_name: &str, wanted: Area) -> Option<CloseUp> {
    let close_up = |screen: &Screen, pages: &[Value]| {
        let (position, page) = pages.iter().enumerate().find(|(position, page)| name_of_page(*position, page) == page_name)?;
        enlarged(&screen.page_drawn(position, page)?, within_the_page(wanted))
    };
    on_the_screen_of(plugin, close_up).flatten()
}

/// Looks at the plugin's screen as it stands: its layout, what was built of it, its pictures and its arrangement.
fn on_the_screen_of<Seen>(plugin: &Path, see: impl FnOnce(&Screen, &[Value]) -> Seen) -> Option<Seen> {
    let layout = library::screen_layout(plugin)?;
    let built = library::built_parameters(plugin).unwrap_or(Value::Null);
    let built_controls = built["params"].as_array().map(Vec::as_slice).unwrap_or_default();
    let screen = Screen { plugin, layout: &layout, built_controls, pictures: drawn_pictures(plugin), arranged: arranged_screen_of(plugin) };
    let pages = layout["pages"].as_array().map(Vec::as_slice).unwrap_or_default();
    Some(see(&screen, pages))
}

/// A page without a name goes by its number.
fn name_of_page(position: usize, page: &Value) -> String {
    page["name"].as_str().map(str::to_string).unwrap_or_else(|| (position + 1).to_string())
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
    Centre,
}

/// How an image is laid in its place.
#[derive(Clone, Copy, PartialEq)]
enum Fit {
    /// At its own size, shrunk only if it is too big to fit: how the MPC shows the generator's own layout.
    OwnSize,
    /// Stretched to the place, for artwork the workbench draws again at that size.
    Filling,
}

struct Lettering {
    words: String,
    place: Area,
    height: f32,
    colour: String,
    align: Align,
}

/// A page being drawn: its pixels so far, and the lettering to lay over them at the end.
struct Canvas {
    pixels: Pixmap,
    lettering: Vec<Lettering>,
    images: HashMap<String, Option<Pixmap>>,
}

impl Canvas {
    fn of_colour(colour: &str) -> Option<Self> {
        let mut pixels = Pixmap::new(WHOLE_PAGE.width, WHOLE_PAGE.height)?;
        pixels.fill(colour_of(colour));
        Some(Self { pixels, lettering: Vec::new(), images: HashMap::new() })
    }

    fn fill(&mut self, place: Area, colour: &str) {
        let mut paint = Paint::default();
        paint.set_color(colour_of(colour));
        if let Some(rectangle) = Rect::from_xywh(place.x as f32, place.y as f32, place.width as f32, place.height as f32) {
            self.pixels.fill_rect(rectangle, &paint, Transform::identity(), None);
        }
    }

    fn ring(&mut self, place: Area, colour: &str) {
        let (centre_x, centre_y) = (place.x as f32 + place.width as f32 / 2.0, place.y as f32 + place.height as f32 / 2.0);
        let stroke = Stroke { width: 8.0, ..Stroke::default() };
        let mut paint = Paint::default();
        paint.set_color(colour_of(colour));
        paint.anti_alias = true;
        if let Some(circle) = PathBuilder::from_circle(centre_x, centre_y, place.width as f32 / 2.0 - stroke.width) {
            self.pixels.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
        }
    }

    /// Lays one frame of an image in the middle of a place.
    fn stamp(&mut self, image: &Pixmap, frame: u32, frames: u32, place: Area, fit: Fit) {
        let frame_height = image.height() / frames.max(1);
        let top = i32::try_from(frame.min(frames.saturating_sub(1)) * frame_height).unwrap_or(0);
        let cut = IntRect::from_xywh(0, top, image.width(), frame_height).and_then(|frame| image.as_ref().clone_rect(frame));
        let Some(cut) = cut else { return };
        let to_fill = (place.width as f32 / cut.width() as f32).min(place.height as f32 / cut.height() as f32);
        let shrink = if fit == Fit::Filling { to_fill } else { to_fill.min(1.0) };
        let left = place.x as f32 + (place.width as f32 - cut.width() as f32 * shrink) / 2.0;
        let top = place.y as f32 + (place.height as f32 - cut.height() as f32 * shrink) / 2.0;
        let placed = Transform::from_scale(shrink, shrink).post_translate(left, top);
        self.pixels.draw_pixmap(0, 0, cut.as_ref(), &smoothly(), placed, None);
    }

    fn letter(&mut self, words: &str, place: Area, height: f32, colour: &str, align: Align) {
        self.lettering.push(Lettering { words: words.to_string(), place, height, colour: colour.to_string(), align });
    }

    /// Lays the lettering over the pixels: the whole page at its real size.
    fn lettered(mut self) -> Pixmap {
        if let Ok(lettering) = Tree::from_str(&svg_of(&self.lettering), &options_with_this_computers_fonts()) {
            resvg::render(&lettering, Transform::identity(), &mut self.pixels.as_mut());
        }
        self.pixels
    }
}

/// The page shrunk to the width it is shown at.
fn shrunk(page: &Pixmap) -> Option<Vec<u8>> {
    let shrink = VIEW_WIDTH as f32 / WHOLE_PAGE.width as f32;
    let mut view = Pixmap::new(VIEW_WIDTH, (WHOLE_PAGE.height as f32 * shrink).round() as u32)?;
    view.draw_pixmap(0, 0, page.as_ref(), &smoothly(), Transform::from_scale(shrink, shrink), None);
    view.encode_png().ok()
}

/// The part of the page that was asked for, kept inside the page and never too small to see.
fn within_the_page(wanted: Area) -> Area {
    let x = wanted.x.min(WHOLE_PAGE.width - SMALLEST_CLOSE_UP);
    let y = wanted.y.min(WHOLE_PAGE.height - SMALLEST_CLOSE_UP);
    area(x, y, wanted.width.clamp(SMALLEST_CLOSE_UP, WHOLE_PAGE.width - x), wanted.height.clamp(SMALLEST_CLOSE_UP, WHOLE_PAGE.height - y))
}

/// One part of the page with every one of its pixels drawn several times over, so nothing is smoothed away.
fn enlarged(page: &Pixmap, part: Area) -> Option<CloseUp> {
    let times = (CLOSE_UP_FITS.0 / part.width).min(CLOSE_UP_FITS.1 / part.height).clamp(1, MOST_ENLARGED);
    let mut view = Pixmap::new(part.width * times, part.height * times)?;
    let pixel_for_pixel = PixmapPaint { quality: FilterQuality::Nearest, ..PixmapPaint::default() };
    let onto_the_view = Transform::from_translate(-(part.x as f32), -(part.y as f32)).post_scale(times as f32, times as f32);
    view.draw_pixmap(0, 0, page.as_ref(), &pixel_for_pixel, onto_the_view, None);
    Some(CloseUp { png: view.encode_png().ok()?, part, times })
}

/// What a control is, as the layout file asks for it.
#[derive(Clone, Copy, PartialEq)]
enum Widget {
    Knob,
    Fader,
    Switch,
    Slider,
}

/// What the built plugin reports about one control, where it rests.
struct BuiltControl<'a> {
    reported: &'a Value,
}

impl BuiltControl<'_> {
    fn amount(&self) -> f32 {
        (self.reported["default"].as_f64().unwrap_or(0.5) as f32).clamp(0.0, 1.0)
    }

    fn value(&self) -> String {
        let parts = [self.reported["display"].as_str().unwrap_or_default().trim(), self.reported["label"].as_str().unwrap_or_default().trim()];
        parts.iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ")
    }

    fn is_on_off(&self) -> bool {
        self.reported["kind"] == "bool"
    }

    fn choices(&self) -> Vec<&str> {
        let listed: Vec<&str> = self.reported["choices"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if listed.is_empty() && self.is_on_off() { vec!["Off", "On"] } else { listed }
    }

    fn chosen(&self) -> usize {
        (self.amount() * self.choices().len().saturating_sub(1) as f32).round() as usize
    }

    /// A centre-zero control rests in the middle and reads as zero there.
    fn rests_at_centre_zero(&self) -> bool {
        self.choices().is_empty() && (self.amount() - 0.5).abs() <= 0.01 && reads_as_zero(self.reported["display"].as_str().unwrap_or_default())
    }
}

/// Whether a value such as "0", "+0 st" or "0.0 dB" is zero, as Akai's generator judges it.
fn reads_as_zero(display: &str) -> bool {
    let unsigned = display.trim().trim_start_matches(['+', '-']);
    let after_zeros = unsigned.trim_start_matches('0');
    let after_number = match after_zeros.strip_prefix('.') {
        Some(decimals) if decimals.starts_with('0') => decimals.trim_start_matches('0'),
        Some(_) => return false,
        None => after_zeros,
    };
    !after_number.contains(|character: char| character.is_ascii_digit() || character == '.')
}

struct Screen<'a> {
    plugin: &'a Path,
    layout: &'a Value,
    built_controls: &'a [Value],
    pictures: Vec<Picture>,
    /// Where everything sits once the musician's own arrangement is applied, page by page.
    arranged: Vec<ArrangedPage>,
}

impl Screen<'_> {
    fn view_of(&self, position: usize, page: &Value) -> Option<PageView> {
        Some(PageView { page_name: name_of_page(position, page), png: shrunk(&self.page_drawn(position, page)?)? })
    }

    /// The whole page at its real size, as the MPC lays it out.
    fn page_drawn(&self, position: usize, page: &Value) -> Option<Pixmap> {
        let page_name = name_of_page(position, page);
        let arranged = self.arranged.get(position);
        let mut canvas = Canvas::of_colour(self.style("background", DEFAULT_BACKGROUND))?;
        self.draw_pictures(&mut canvas, &page_name, Depth::Back);
        self.draw_page_art(&mut canvas, &page_name);
        self.draw_pictures(&mut canvas, &page_name, Depth::Behind);
        let slots = page["controls"].as_array().map(Vec::as_slice).unwrap_or_default();
        for (slot, entry) in slots.iter().take(SLOTS_ON_A_PAGE).enumerate() {
            let as_arranged = arranged.and_then(|page| page.items.iter().find(|item| item.slot == slot));
            self.draw_slot(&mut canvas, SlotAsked { slot, entry, as_arranged });
        }
        self.draw_pictures(&mut canvas, &page_name, Depth::Front);
        Some(canvas.lettered())
    }

    fn style<'s>(&'s self, name: &str, otherwise: &'s str) -> &'s str {
        self.layout["style"][name].as_str().filter(|colour| colour.len() == 8).unwrap_or(otherwise)
    }

    fn has_kit(&self) -> bool {
        !self.layout["kit"].is_null()
    }

    fn accent(&self) -> &str {
        let kit = &self.layout["kit"];
        let theme = kit.as_str().or(kit["theme"].as_str()).unwrap_or(DEFAULT_KIT);
        let of_theme = KIT_COLOURS.iter().find(|(name, _)| *name == theme).map_or(KIT_COLOURS[0].1, |(_, colour)| *colour);
        kit["accent"].as_str().or(self.layout["style"]["accent"].as_str()).filter(|colour| colour.len() == 8).unwrap_or(of_theme)
    }

    fn built(&self, name: &str) -> Option<BuiltControl<'_>> {
        let same_name = |control: &&Value| control["name"].as_str().is_some_and(|known| known.eq_ignore_ascii_case(name));
        self.built_controls.iter().find(same_name).map(|reported| BuiltControl { reported })
    }

    /// The artwork the musician laid behind the page's controls, in front of any backdrop picture.
    fn draw_page_art(&self, canvas: &mut Canvas, page_name: &str) {
        let art = page_art_image(self.plugin, page_name).and_then(|png| Pixmap::decode_png(&png).ok());
        if let Some(art) = art {
            canvas.stamp(&art, 0, 1, WHOLE_PAGE, Fit::OwnSize);
        }
    }

    /// The page's pictures that sit at one depth, each where the musician put it and as solid as they made it.
    fn draw_pictures(&self, canvas: &mut Canvas, page_name: &str, depth: Depth) {
        let Some(page) = self.arranged.iter().find(|page| page.name == page_name) else { return };
        for arranged in page.pictures.iter().filter(|arranged| arranged.depth == depth) {
            let picture = self.pictures.iter().find(|picture| picture.page == page_name && picture.name == arranged.name);
            let (Some(picture), Some(image)) = (picture, self.drawn(arranged)) else { continue };
            canvas.stamp(&image, self.frame_shown_of(picture), picture.frames, arranged.area, Fit::OwnSize);
        }
    }

    fn drawn(&self, arranged: &ArrangedPicture) -> Option<Pixmap> {
        let solid = fs::read(drawn_file_of(self.plugin, &arranged.name)).ok()?;
        let shown = if arranged.opacity < FULLY_SOLID { faded(&solid, arranged.opacity).ok()? } else { solid };
        Pixmap::decode_png(&shown).ok()
    }

    /// A picture that follows a control shows the frame for where that control rests. Any other shows its first.
    fn frame_shown_of(&self, picture: &Picture) -> u32 {
        let Motion::Follows(control) = &picture.motion else { return 0 };
        let amount = self.built(control).map_or(0.0, |built| built.amount());
        (amount * picture.frames.saturating_sub(1) as f32).round() as u32
    }

    fn draw_slot(&self, canvas: &mut Canvas, asked: SlotAsked) {
        let corner = asked.area();
        let within = |part: Area| area(corner.x + part.x, corner.y + part.y, part.width, part.height);
        let entry = asked.entry;
        let widget = entry["widget"].as_str().unwrap_or("slider");
        match (entry.as_str(), widget) {
            (Some("empty"), _) | (None, "empty") => {}
            (None, "text") => self.draw_heading(canvas, &asked),
            (bare_name, _) => {
                let name = bare_name.or(entry["param"].as_str()).unwrap_or_default();
                let control = ControlAsked { widget: widget_named(widget), name, shown_name: asked.shown_as(name), polarity: entry["polarity"].as_str(), parts: asked.parts() };
                self.draw_control(canvas, &within, control);
            }
        }
    }

    /// A heading is lettered the way the layout's style says, or the way the musician lettered it by hand.
    fn draw_heading(&self, canvas: &mut Canvas, asked: &SlotAsked) {
        let corner = asked.area();
        let words = asked.shown_as(asked.entry["text"].as_str().unwrap_or_default());
        match asked.parts().and_then(|parts| parts.name.as_ref()) {
            Some(label) => {
                let place = area(corner.x + label.area.x, corner.y + label.area.y, label.area.width, label.area.height);
                canvas.letter(words, place, label.height as f32, &argb_of(label), align_of(label));
            }
            None => canvas.letter(words, corner, 30.0, self.style("text", DEFAULT_TEXT), Align::Left),
        }
    }

    fn draw_control(&self, canvas: &mut Canvas, within: &dyn Fn(Area) -> Area, asked: ControlAsked) {
        let built = self.built(asked.name);
        let pen = ControlPen { screen: self, canvas, within, shown_name: asked.shown_name, built: built.as_ref(), parts: asked.parts };
        let centre_zero = match asked.polarity {
            Some("bipolar") => true,
            Some("unipolar") => false,
            _ => built.as_ref().is_some_and(BuiltControl::rests_at_centre_zero),
        };
        let buttons = built.as_ref().filter(|built| !built.is_on_off()).and_then(|built| places::choice_buttons(built.choices().len()));
        match (asked.widget, buttons) {
            (Widget::Switch, Some((width, lefts))) => pen.choice(width, lefts),
            (Widget::Switch, None) => pen.toggle(),
            (Widget::Knob, _) => pen.knob(centre_zero),
            (Widget::Fader, _) => pen.fader(centre_zero),
            (Widget::Slider, _) => pen.slider(),
        }
    }
}

/// One slot of the layout, with what the musician changed about it, if anything.
struct SlotAsked<'a> {
    slot: usize,
    entry: &'a Value,
    as_arranged: Option<&'a ArrangedItem>,
}

impl<'a> SlotAsked<'a> {
    fn area(&self) -> Area {
        self.as_arranged.map_or_else(|| area_of_slot(self.slot), |item| item.area)
    }

    /// How the musician laid out its inside by hand, if they did.
    fn parts(&self) -> Option<&'a Parts> {
        self.as_arranged.and_then(|item| item.parts.as_ref())
    }

    /// The name the musician gave it, or failing that its own.
    fn shown_as(&self, own_name: &'a str) -> &'a str {
        self.as_arranged.and_then(|item| item.shown_name.as_deref()).unwrap_or(own_name)
    }
}

struct ControlAsked<'a> {
    widget: Widget,
    /// The name the plugin knows the control by.
    name: &'a str,
    /// The name written beside it on the screen.
    shown_name: &'a str,
    polarity: Option<&'a str>,
    parts: Option<&'a Parts>,
}

/// A colour the musician picked, written the way layout files write theirs: alpha first.
fn argb_of(label: &LabelLook) -> String {
    format!("ff{}", label.colour.written().trim_start_matches('#'))
}

fn align_of(label: &LabelLook) -> Align {
    match label.align {
        control_parts::Align::Left => Align::Left,
        control_parts::Align::Centre => Align::Centre,
        control_parts::Align::Right => Align::Right,
    }
}

fn widget_named(widget: &str) -> Widget {
    match widget {
        "knob" => Widget::Knob,
        "fader" => Widget::Fader,
        "switch" => Widget::Switch,
        _ => Widget::Slider,
    }
}

/// Draws one control into its slot: its artwork from the build, and its name and value in the page's colours.
struct ControlPen<'a> {
    screen: &'a Screen<'a>,
    canvas: &'a mut Canvas,
    within: &'a dyn Fn(Area) -> Area,
    shown_name: &'a str,
    built: Option<&'a BuiltControl<'a>>,
    /// Where the musician put the control's parts by hand. Without it, each part sits where the generator puts it.
    parts: Option<&'a Parts>,
}

impl ControlPen<'_> {
    fn amount(&self) -> f32 {
        self.built.map_or(0.5, BuiltControl::amount)
    }

    /// Letters a name or a value: the way the musician laid it out by hand, or else the generator's way.
    fn letter(&mut self, words: &str, by_hand: Option<&LabelLook>, generated: Lettering) {
        match by_hand {
            Some(label) if label.hidden => {}
            Some(label) => self.canvas.letter(words, (self.within)(label.area), label.height as f32, &argb_of(label), align_of(label)),
            None => self.canvas.letter(words, (self.within)(generated.place), generated.height, &generated.colour, generated.align),
        }
    }

    fn name_at(&mut self, place: Area, height: f32) {
        let muted = self.screen.style("muted", DEFAULT_MUTED).to_string();
        let by_hand = self.parts.and_then(|parts| parts.name.as_ref());
        self.letter(self.shown_name, by_hand, Lettering { words: String::new(), place, height, colour: muted, align: Align::Left });
    }

    fn value_at(&mut self, place: Area, height: f32, align: Align) {
        let text = self.screen.style("text", DEFAULT_TEXT).to_string();
        let value = self.built.map(BuiltControl::value).unwrap_or_default();
        let by_hand = self.parts.and_then(|parts| parts.value.as_ref());
        self.letter(&value, by_hand, Lettering { words: String::new(), place, height, colour: text, align });
    }

    /// Where the image that draws the control goes, and whether it is stretched to a size the musician chose.
    fn art_place(&self, generated: Area) -> (Area, Fit) {
        match self.parts.and_then(|parts| parts.art) {
            Some(by_hand) => (by_hand, Fit::Filling),
            None => (generated, Fit::OwnSize),
        }
    }

    fn kit_image(&mut self, file: &str, frame: u32, frames: u32, place: Area) {
        self.kit_image_fitted(file, frame, frames, (place, Fit::OwnSize));
    }

    fn kit_image_fitted(&mut self, file: &str, frame: u32, frames: u32, (place, fit): (Area, Fit)) {
        let plugin = self.screen.plugin;
        let decoded = || built_skin_image(plugin, file).and_then(|png| Pixmap::decode_png(&png).ok());
        let image = self.canvas.images.entry(file.to_string()).or_insert_with(decoded).clone();
        if let Some(image) = image {
            self.canvas.stamp(&image, frame, frames, (self.within)(place), fit);
        }
    }

    fn kit_strip(&mut self, file: &str, place: Area) {
        let frame = (self.amount() * (KIT_FRAMES - 1) as f32).round() as u32;
        self.kit_image_fitted(file, frame, KIT_FRAMES, self.art_place(place));
    }

    fn knob(mut self, centre_zero: bool) {
        if self.screen.has_kit() {
            self.kit_strip(if centre_zero { "knob_bi.png" } else { "knob_uni.png" }, places::KIT_KNOB);
            self.name_at(places::KIT_KNOB_NAME, 26.0);
            self.value_at(places::KIT_KNOB_VALUE, 30.0, Align::Left);
        } else {
            self.canvas.ring((self.within)(self.art_place(places::PLAIN_KNOB).0), TRACK_GREY);
            self.name_at(places::PLAIN_KNOB_NAME, 26.0);
            self.value_at(places::PLAIN_KNOB_VALUE, 30.0, Align::Left);
        }
    }

    fn fader(mut self, centre_zero: bool) {
        self.name_at(places::FADER_NAME, 24.0);
        self.value_at(places::FADER_VALUE, 26.0, Align::Right);
        self.kit_strip(if centre_zero { "fader_h_bi.png" } else { "fader_h_uni.png" }, places::FADER);
    }

    fn toggle(mut self) {
        let on = self.amount() >= 0.5;
        self.name_at(places::TOGGLE_NAME, 26.0);
        self.value_at(places::TOGGLE_VALUE, 30.0, Align::Left);
        let place = self.art_place(places::TOGGLE).0;
        self.kit_image(if on { "toggle_on.png" } else { "toggle_off.png" }, 0, 1, area(place.x, place.y, places::TOGGLE.width, places::TOGGLE.height));
    }

    fn choice(mut self, width: u32, lefts: &[u32]) {
        let (choices, chosen) = self.built.map(|built| (built.choices(), built.chosen())).unwrap_or_default();
        let text = self.screen.style("text", DEFAULT_TEXT).to_string();
        self.name_at(places::CHOICE_NAME, 24.0);
        for (step, (choice, left)) in choices.iter().zip(lefts).enumerate() {
            let place = area(*left, places::CHOICE_TOP, width, places::CHOICE_HEIGHT);
            let state = if step == chosen { "on" } else { "off" };
            self.kit_image(&format!("seg{}_{state}.png", choices.len()), 0, 1, place);
            self.canvas.letter(choice, (self.within)(place), 22.0, &text, Align::Centre);
        }
    }

    /// The MPC's standard slider: name and value above a bar. Its exact look is the MPC's own, so this is close, not exact.
    fn slider(mut self) {
        let bar = (self.within)(places::SLIDER_BAR);
        let filled = area(bar.x, bar.y, (bar.width as f32 * self.amount()).round() as u32, bar.height);
        let accent = self.screen.accent().to_string();
        self.name_at(places::SLIDER_NAME, 24.0);
        self.value_at(places::SLIDER_VALUE, 26.0, Align::Right);
        self.canvas.fill(bar, TRACK_GREY);
        self.canvas.fill(filled, &accent);
    }
}

/// A colour written alpha first, as layout files write it: AARRGGBB. Anything else is black.
fn colour_of(argb: &str) -> Color {
    let channel = |start: usize| argb.get(start..start + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()).unwrap_or(0);
    if argb.len() == 8 { Color::from_rgba8(channel(2), channel(4), channel(6), channel(0)) } else { Color::BLACK }
}

fn smoothly() -> PixmapPaint {
    PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() }
}

/// The page's lettering as a drawing of its own. The words come from files the AI wrote, so they are
/// written as plain text and can never be read as markup.
fn svg_of(lettering: &[Lettering]) -> String {
    let lines: String = lettering.iter().map(text_element).collect();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">{lines}</svg>"#, WHOLE_PAGE.width, WHOLE_PAGE.height)
}

fn text_element(lettering: &Lettering) -> String {
    let size = lettering.height * LETTER_SIZE_FOR_HEIGHT;
    let place = lettering.place;
    let (x, anchor) = match lettering.align {
        Align::Left => (place.x as f32, "start"),
        Align::Right => ((place.x + place.width) as f32, "end"),
        Align::Centre => (place.x as f32 + place.width as f32 / 2.0, "middle"),
    };
    let baseline = place.y as f32 + place.height as f32 / 2.0 + size * 0.35;
    let colour = lettering.colour.get(2..).unwrap_or("ffffff");
    format!(
        r##"<text x="{x}" y="{baseline}" font-family="{FONTS}" font-size="{size}" fill="#{colour}" text-anchor="{anchor}">{}</text>"##,
        as_plain_text(&lettering.words)
    )
}

fn as_plain_text(words: &str) -> String {
    words.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::film_strip::test_support::{pixel_of, size_of};
    use crate::workspace::test_support::TemporaryFolder;

    const PAGE_COLOUR: [u8; 4] = [0x20, 0x30, 0x40, 255];
    const LAYOUT: &str = r#"{ "style": { "background": "ff203040" }, "pages": [
        { "name": "MAIN", "controls": ["Level", "empty", { "widget": "text", "text": "TONE <b>" }] },
        { "controls": [] }
    ] }"#;
    const BUILT: &str = r#"{ "params": [{ "name": "Level", "default": 0.5, "display": "50", "label": "%", "kind": "float", "choices": [] }] }"#;

    fn plugin_with_a_layout() -> TemporaryFolder {
        let plugin = TemporaryFolder::create("page-view");
        fs::create_dir_all(plugin.0.join("build")).unwrap();
        fs::write(plugin.0.join("ui.json"), LAYOUT).unwrap();
        fs::write(plugin.0.join("build/params.json"), BUILT).unwrap();
        plugin
    }

    /// Where a pixel of the full page lands once the page is shrunk to the width it is shown at.
    fn in_view(x: u32, y: u32) -> (u32, u32) {
        (x * VIEW_WIDTH / WHOLE_PAGE.width, y * VIEW_WIDTH / WHOLE_PAGE.width)
    }

    #[test]
    fn every_page_is_drawn_at_the_mpcs_shape_in_the_pages_own_colour() {
        let plugin = plugin_with_a_layout();

        let views = page_views(&plugin.0);

        let names: Vec<&str> = views.iter().map(|view| view.page_name.as_str()).collect();
        assert_eq!(names, ["MAIN", "2"]);
        assert_eq!(size_of(&views[0].png), (1024, 502));
        assert_eq!(pixel_of(&views[1].png, 500, 250), PAGE_COLOUR);
    }

    #[test]
    fn a_slider_shows_how_far_it_is_turned_in_the_kits_colour() {
        let plugin = plugin_with_a_layout();

        let main = &page_views(&plugin.0)[0];

        let (filled_x, bar_y) = in_view(23 + 15 + 40, 45 + 58 + 11);
        let (unfilled_x, _) = in_view(23 + 15 + 200, 0);
        assert_eq!(pixel_of(&main.png, filled_x, bar_y), [0xd3, 0x11, 0x45, 255]);
        assert_eq!(pixel_of(&main.png, unfilled_x, bar_y), [0x43, 0x43, 0x47, 255]);
    }

    #[test]
    fn a_control_the_musician_moved_is_drawn_where_they_put_it() {
        let plugin = plugin_with_a_layout();
        let moved = r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Level", "place": { "x": 700, "y": 400 } }] }] }"#;
        fs::write(plugin.0.join("look.json"), moved).unwrap();

        let main = &page_views(&plugin.0)[0];

        let (filled_x, bar_y) = in_view(700 + 15 + 40, 400 + 58 + 11);
        let (old_x, old_y) = in_view(23 + 15 + 40, 45 + 58 + 11);
        assert_eq!(pixel_of(&main.png, filled_x, bar_y), [0xd3, 0x11, 0x45, 255]);
        assert_eq!(pixel_of(&main.png, old_x, old_y), PAGE_COLOUR);
    }

    #[test]
    fn words_from_the_layout_are_lettered_as_plain_text_never_as_markup() {
        let heading = Lettering { words: "TONE <b> & more".to_string(), place: WHOLE_PAGE, height: 30.0, colour: DEFAULT_TEXT.to_string(), align: Align::Left };

        let drawing = svg_of(&[heading]);

        assert!(drawing.contains(">TONE &lt;b&gt; &amp; more</text>"), "{drawing}");
        assert!(Tree::from_str(&drawing, &resvg::usvg::Options::default()).is_ok());
    }

    #[test]
    fn zero_is_recognised_the_way_akais_generator_does() {
        assert!(reads_as_zero("0") && reads_as_zero("0.0 dB") && reads_as_zero("+0 st") && reads_as_zero(""));
        assert!(!reads_as_zero("50 %") && !reads_as_zero("0.5") && !reads_as_zero("10"));
    }

    #[test]
    fn a_plugin_without_a_layout_has_no_pages_to_show() {
        let plugin = TemporaryFolder::create("page-view-none");

        assert!(page_views(&plugin.0).is_empty());
    }
}
