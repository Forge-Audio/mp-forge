//! The artwork the musician lays behind a page's controls by hand: blocks of colour, words and pictures.
//!
//! Each page of a look keeps its artwork as a list of layers, from the back of the page to the front.
//! The MPC never sees the layers themselves: they are drawn into one background picture for the page.

use serde_json::{Map, Value, json};

use crate::art_shelf::is_art_file_name;
use crate::screen_layout::Area;

pub const MOST_LAYERS_ON_A_PAGE: usize = 60;
/// A layer may hang off the page, as in any picture editor, but only so far.
const FURTHEST_FROM_THE_PAGE: i64 = 4096;
pub const LONGEST_SIDE: u32 = 2560;
const LONGEST_WORDS: usize = 120;
const LONGEST_FONT_NAME: usize = 64;
const SMALLEST_LETTERS: u32 = 6;
const TALLEST_LETTERS: u32 = 400;
const WIDEST_EDGE: u32 = 64;
const ROUNDEST_CORNER: u32 = 1280;
const FULLY_SOLID: u32 = 100;
/// A fade's ends are given in hundredths of the block's size, and may lie a little outside it.
const FURTHEST_FADE_END: i64 = 400;
const PUNCTUATION_IN_FONT_NAMES: &str = " -_.";

const BLOCK: &str = "block";
const WORDS: &str = "words";
const PICTURE: &str = "picture";
const ELLIPSE: &str = "ellipse";
const RECTANGLE: &str = "rectangle";
const SHARED_FIELDS: [&str; 5] = ["kind", "id", "opacity", "hidden", "tilt"];
/// A layer can be tilted half a turn either way, which reaches every angle.
const FURTHEST_TILT: i64 = 180;
const BLOCK_FIELDS: [&str; 6] = ["shape", "area", "colour", "corner", "fade", "edge"];
const WORDS_FIELDS: [&str; 6] = ["text", "font", "size", "bold", "colour", "place"];
const PICTURE_FIELDS: [&str; 3] = ["file", "area", "crop"];

/// The pixels a layer covers on the page. Unlike a control, a layer may start left of or above the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Colour {
    /// A colour written the way a web page writes it: #rrggbb.
    pub fn written_as(text: &str) -> Option<Colour> {
        let digits = text.strip_prefix('#').filter(|digits| digits.len() == 6 && digits.is_ascii())?;
        let channel = |start: usize| u8::from_str_radix(&digits[start..start + 2], 16).ok();
        Some(Colour { red: channel(0)?, green: channel(2)?, blue: channel(4)? })
    }

    pub fn written(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rectangle,
    Ellipse,
}

/// A point inside a block, in hundredths of the block's width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    pub x: i32,
    pub y: i32,
}

/// The block's own colour at `from` turning smoothly into `colour` at `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    pub colour: Colour,
    pub from: Spot,
    pub to: Spot,
}

/// A line drawn round the inside of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub colour: Colour,
    pub width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub shape: Shape,
    pub frame: Frame,
    pub colour: Colour,
    /// How round a rectangle's corners are, in pixels. Nothing for square corners.
    pub corner: u32,
    pub fade: Option<Fade>,
    pub edge: Option<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Words {
    pub text: String,
    /// The name of a font on the musician's computer.
    pub font: String,
    /// The height of the letters in pixels.
    pub size: u32,
    pub bold: bool,
    pub colour: Colour,
    /// The top left corner of the line of words.
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedArt {
    /// One of the pictures the musician brought in, by the name it was kept under.
    pub file: String,
    pub frame: Frame,
    /// The part of the picture to show, in the picture's own pixels. Nothing shows all of it.
    pub crop: Option<Area>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Art {
    Block(Block),
    Words(Words),
    Picture(PlacedArt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    /// Tells the layers of one page apart. It stays the same however the layer is changed.
    pub id: u32,
    pub hidden: bool,
    /// From 0, which cannot be seen, to 100, which hides whatever is behind it.
    pub opacity: u32,
    /// How far it is turned about its own middle, in degrees clockwise. Nothing for upright.
    pub tilt: i32,
    pub art: Art,
}

pub fn layers_from(layers: &Value) -> Result<Vec<Layer>, String> {
    let listed = layers.as_array().ok_or("The \"artwork\" of a page is a list of layers.")?;
    if listed.len() > MOST_LAYERS_ON_A_PAGE {
        return Err(format!("A page can hold {MOST_LAYERS_ON_A_PAGE} layers of artwork at most."));
    }
    let layers: Vec<Layer> = listed.iter().map(layer_from).collect::<Result<_, _>>()?;
    match repeated_id(&layers) {
        Some(id) => Err(format!("Two layers of a page share the id {id}.")),
        None => Ok(layers),
    }
}

fn repeated_id(layers: &[Layer]) -> Option<u32> {
    let ids = layers.iter().map(|layer| layer.id);
    ids.enumerate().find(|(position, id)| layers[..*position].iter().any(|earlier| earlier.id == *id)).map(|(_, id)| id)
}

pub fn layer_from(layer: &Value) -> Result<Layer, String> {
    let fields = layer.as_object().ok_or("A layer of artwork is written between curly brackets.")?;
    let kind = fields.get("kind").and_then(Value::as_str).unwrap_or_default();
    let id = whole(fields, "id").and_then(|id| u32::try_from(id).ok()).ok_or("Every layer needs a whole number as its \"id\".")?;
    let about = |problem: String| format!("Layer {id}: {problem}");
    let art = match kind {
        BLOCK => known(fields, &BLOCK_FIELDS).and_then(block_from).map(Art::Block),
        WORDS => known(fields, &WORDS_FIELDS).and_then(words_from).map(Art::Words),
        PICTURE => known(fields, &PICTURE_FIELDS).and_then(placed_art_from).map(Art::Picture),
        _ => Err("its \"kind\" is \"block\", \"words\" or \"picture\".".to_string()),
    };
    Ok(Layer {
        id,
        hidden: fields.get("hidden") == Some(&Value::Bool(true)),
        opacity: opacity_from(fields).map_err(about)?,
        tilt: tilt_from(fields).map_err(about)?,
        art: art.map_err(about)?,
    })
}

pub fn layer_as_json(layer: &Layer) -> Value {
    let mut written = match &layer.art {
        Art::Block(block) => block_as_json(block),
        Art::Words(words) => words_as_json(words),
        Art::Picture(picture) => placed_art_as_json(picture),
    };
    written["id"] = json!(layer.id);
    if layer.opacity != FULLY_SOLID {
        written["opacity"] = json!(layer.opacity);
    }
    if layer.hidden {
        written["hidden"] = json!(true);
    }
    if layer.tilt != 0 {
        written["tilt"] = json!(layer.tilt);
    }
    written
}

type Fields = Map<String, Value>;

/// The fields of a layer, once none of them is one its kind does not have.
fn known<'a>(fields: &'a Fields, of_its_kind: &[&str]) -> Result<&'a Fields, String> {
    let is_known = |key: &str| SHARED_FIELDS.contains(&key) || of_its_kind.contains(&key);
    match fields.keys().find(|key| !is_known(key)) {
        Some(stranger) => Err(format!("it has no \"{stranger}\". It has: {}.", of_its_kind.join(", "))),
        None => Ok(fields),
    }
}

fn whole(fields: &Fields, name: &str) -> Option<i64> {
    fields.get(name).and_then(Value::as_i64)
}

fn whole_between(fields: &Fields, name: &str, least: i64, most: i64) -> Option<i64> {
    whole(fields, name).filter(|number| (least..=most).contains(number))
}

fn opacity_from(fields: &Fields) -> Result<u32, String> {
    match fields.get("opacity") {
        None => Ok(FULLY_SOLID),
        Some(_) => whole_between(fields, "opacity", 0, i64::from(FULLY_SOLID)).map(|opacity| opacity as u32).ok_or_else(|| "its \"opacity\" is a whole number from 0 to 100.".to_string()),
    }
}

fn tilt_from(fields: &Fields) -> Result<i32, String> {
    match fields.get("tilt") {
        None => Ok(0),
        Some(_) => whole_between(fields, "tilt", -FURTHEST_TILT, FURTHEST_TILT).map(|degrees| degrees as i32).ok_or_else(|| format!("its \"tilt\" is whole degrees from -{FURTHEST_TILT} to {FURTHEST_TILT}.")),
    }
}

fn colour_from(fields: &Fields, name: &str) -> Result<Colour, String> {
    let written = fields.get(name).and_then(Value::as_str);
    written.and_then(Colour::written_as).ok_or_else(|| format!("its \"{name}\" is written like #1e3a28."))
}

fn frame_from(fields: &Fields) -> Result<Frame, String> {
    let wrong = || format!("its \"area\" is whole pixels, such as {{\"x\": 0, \"y\": 0, \"width\": 1280, \"height\": 628}}, no longer than {LONGEST_SIDE} on a side.");
    let area = fields.get("area").and_then(Value::as_object).ok_or_else(wrong)?;
    let corner = |name: &str| whole_between(area, name, -FURTHEST_FROM_THE_PAGE, FURTHEST_FROM_THE_PAGE).map(|pixels| pixels as i32);
    let side = |name: &str| whole_between(area, name, 1, i64::from(LONGEST_SIDE)).map(|pixels| pixels as u32);
    match (corner("x"), corner("y"), side("width"), side("height")) {
        (Some(x), Some(y), Some(width), Some(height)) if area.len() == 4 => Ok(Frame { x, y, width, height }),
        _ => Err(wrong()),
    }
}

fn block_from(fields: &Fields) -> Result<Block, String> {
    let shape = match fields.get("shape").and_then(Value::as_str) {
        None | Some(RECTANGLE) => Shape::Rectangle,
        Some(ELLIPSE) => Shape::Ellipse,
        Some(_) => return Err("its \"shape\" is \"rectangle\" or \"ellipse\".".to_string()),
    };
    let corner = match fields.get("corner") {
        None => 0,
        Some(_) => whole_between(fields, "corner", 0, i64::from(ROUNDEST_CORNER)).ok_or("its \"corner\" is how round the corners are, in whole pixels.")? as u32,
    };
    Ok(Block {
        shape,
        frame: frame_from(fields)?,
        colour: colour_from(fields, "colour")?,
        corner,
        fade: fields.get("fade").map(fade_from).transpose()?,
        edge: fields.get("edge").map(edge_from).transpose()?,
    })
}

fn fade_from(fade: &Value) -> Result<Fade, String> {
    let wrong = || "its \"fade\" has a \"colour\" and two ends, \"from\" and \"to\", in hundredths of the block's size.".to_string();
    let fields = fade.as_object().filter(|fields| fields.len() == 3).ok_or_else(wrong)?;
    let end = |name: &str| fields.get(name).and_then(Value::as_object).and_then(spot_from);
    match (end("from"), end("to")) {
        (Some(from), Some(to)) => Ok(Fade { colour: colour_from(fields, "colour")?, from, to }),
        _ => Err(wrong()),
    }
}

fn spot_from(spot: &Fields) -> Option<Spot> {
    let hundredths = |name: &str| whole_between(spot, name, -FURTHEST_FADE_END, FURTHEST_FADE_END).map(|hundredths| hundredths as i32);
    Some(Spot { x: hundredths("x")?, y: hundredths("y")? }).filter(|_| spot.len() == 2)
}

fn edge_from(edge: &Value) -> Result<Edge, String> {
    let wrong = || format!("its \"edge\" has a \"colour\" and a \"width\" from 1 to {WIDEST_EDGE} pixels.");
    let fields = edge.as_object().filter(|fields| fields.len() == 2).ok_or_else(wrong)?;
    let width = whole_between(fields, "width", 1, i64::from(WIDEST_EDGE)).ok_or_else(wrong)?;
    Ok(Edge { colour: colour_from(fields, "colour")?, width: width as u32 })
}

fn words_from(fields: &Fields) -> Result<Words, String> {
    let text = fields.get("text").and_then(Value::as_str).unwrap_or_default();
    if text.trim().is_empty() || text.chars().count() > LONGEST_WORDS || text.chars().any(char::is_control) {
        return Err(format!("its \"text\" is one line of up to {LONGEST_WORDS} characters."));
    }
    let size = whole_between(fields, "size", i64::from(SMALLEST_LETTERS), i64::from(TALLEST_LETTERS));
    let size = size.ok_or_else(|| format!("its \"size\" is the height of its letters, from {SMALLEST_LETTERS} to {TALLEST_LETTERS} pixels."))?;
    let place = fields.get("place").and_then(Value::as_object).filter(|place| place.len() == 2);
    let corner = |name: &str| place.and_then(|place| whole_between(place, name, -FURTHEST_FROM_THE_PAGE, FURTHEST_FROM_THE_PAGE));
    let (Some(x), Some(y)) = (corner("x"), corner("y")) else {
        return Err("its \"place\" is whole pixels from the page's top left corner, such as {\"x\": 400, \"y\": 120}.".to_string());
    };
    Ok(Words {
        text: text.to_string(),
        font: font_name_from(fields)?,
        size: size as u32,
        bold: fields.get("bold") == Some(&Value::Bool(true)),
        colour: colour_from(fields, "colour")?,
        x: x as i32,
        y: y as i32,
    })
}

fn font_name_from(fields: &Fields) -> Result<String, String> {
    let name = fields.get("font").and_then(Value::as_str).unwrap_or_default();
    if is_plain_font_name(name) { Ok(name.to_string()) } else { Err("its \"font\" is the name of a font on this computer.".to_string()) }
}

/// A font's name goes into a drawing as it is, so it may only hold letters, numbers and a little punctuation.
pub fn is_plain_font_name(name: &str) -> bool {
    let allowed = |letter: char| letter.is_alphanumeric() || PUNCTUATION_IN_FONT_NAMES.contains(letter);
    !name.trim().is_empty() && name.chars().count() <= LONGEST_FONT_NAME && name.chars().all(allowed)
}

fn placed_art_from(fields: &Fields) -> Result<PlacedArt, String> {
    let file = fields.get("file").and_then(Value::as_str).filter(|file| is_art_file_name(file));
    let file = file.ok_or("its \"file\" is one of the pictures brought into the editor.")?;
    Ok(PlacedArt { file: file.to_string(), frame: frame_from(fields)?, crop: fields.get("crop").map(crop_from).transpose()? })
}

fn crop_from(crop: &Value) -> Result<Area, String> {
    let wrong = || "its \"crop\" is the part of the picture to show, in the picture's own whole pixels.".to_string();
    let fields = crop.as_object().filter(|fields| fields.len() == 4).ok_or_else(wrong)?;
    let pixels = |name: &str, least: i64| whole_between(fields, name, least, i64::from(LONGEST_SIDE)).map(|pixels| pixels as u32);
    match (pixels("x", 0), pixels("y", 0), pixels("width", 1), pixels("height", 1)) {
        (Some(x), Some(y), Some(width), Some(height)) => Ok(Area { x, y, width, height }),
        _ => Err(wrong()),
    }
}

fn frame_as_json(frame: &Frame) -> Value {
    json!({ "x": frame.x, "y": frame.y, "width": frame.width, "height": frame.height })
}

fn block_as_json(block: &Block) -> Value {
    let shape = if block.shape == Shape::Ellipse { ELLIPSE } else { RECTANGLE };
    let mut written = json!({ "kind": BLOCK, "shape": shape, "area": frame_as_json(&block.frame), "colour": block.colour.written() });
    if block.corner > 0 {
        written["corner"] = json!(block.corner);
    }
    if let Some(fade) = &block.fade {
        written["fade"] = json!({ "colour": fade.colour.written(), "from": { "x": fade.from.x, "y": fade.from.y }, "to": { "x": fade.to.x, "y": fade.to.y } });
    }
    if let Some(edge) = &block.edge {
        written["edge"] = json!({ "colour": edge.colour.written(), "width": edge.width });
    }
    written
}

fn words_as_json(words: &Words) -> Value {
    let mut written = json!({ "kind": WORDS, "text": words.text, "font": words.font, "size": words.size, "colour": words.colour.written(), "place": { "x": words.x, "y": words.y } });
    if words.bold {
        written["bold"] = json!(true);
    }
    written
}

fn placed_art_as_json(picture: &PlacedArt) -> Value {
    let mut written = json!({ "kind": PICTURE, "file": picture.file, "area": frame_as_json(&picture.frame) });
    if let Some(crop) = &picture.crop {
        written["crop"] = json!({ "x": crop.x, "y": crop.y, "width": crop.width, "height": crop.height });
    }
    written
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    pub const RED: Colour = Colour { red: 255, green: 0, blue: 0 };
    pub const BLUE: Colour = Colour { red: 0, green: 0, blue: 255 };

    pub fn solid(id: u32, art: Art) -> Layer {
        Layer { id, hidden: false, opacity: FULLY_SOLID, tilt: 0, art }
    }

    pub fn block_over(frame: Frame, colour: Colour) -> Block {
        Block { shape: Shape::Rectangle, frame, colour, corner: 0, fade: None, edge: None }
    }

    pub fn red_block(id: u32, frame: Frame) -> Layer {
        solid(id, Art::Block(block_over(frame, RED)))
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{BLUE, RED, block_over, red_block, solid};
    use super::*;

    const THREE_LAYERS: &str = r##"[
        { "kind": "block", "id": 1, "shape": "rectangle", "area": { "x": -10, "y": 0, "width": 1300, "height": 628 }, "colour": "#ff0000",
          "corner": 12, "fade": { "colour": "#0000ff", "from": { "x": 0, "y": 0 }, "to": { "x": 0, "y": 100 } }, "edge": { "colour": "#0000ff", "width": 3 } },
        { "kind": "words", "id": 2, "text": "DUSTY & TAPE", "font": "Helvetica Neue", "size": 48, "bold": true, "colour": "#ff0000", "place": { "x": 40, "y": 20 }, "opacity": 80 },
        { "kind": "picture", "id": 5, "file": "art-3.png", "area": { "x": 0, "y": 0, "width": 640, "height": 314 }, "crop": { "x": 10, "y": 20, "width": 300, "height": 150 }, "hidden": true }
    ]"##;

    fn three_layers() -> Vec<Layer> {
        layers_from(&serde_json::from_str(THREE_LAYERS).unwrap()).unwrap()
    }

    fn refusal_of(layer: &str) -> String {
        layer_from(&serde_json::from_str(layer).unwrap()).unwrap_err()
    }

    #[test]
    fn each_kind_of_layer_is_read_with_everything_said_about_it() {
        let layers = three_layers();

        let fade = Fade { colour: BLUE, from: Spot { x: 0, y: 0 }, to: Spot { x: 0, y: 100 } };
        let block = Block { corner: 12, fade: Some(fade), edge: Some(Edge { colour: BLUE, width: 3 }), ..block_over(Frame { x: -10, y: 0, width: 1300, height: 628 }, RED) };
        let words = Words { text: "DUSTY & TAPE".to_string(), font: "Helvetica Neue".to_string(), size: 48, bold: true, colour: RED, x: 40, y: 20 };
        let picture = PlacedArt { file: "art-3.png".to_string(), frame: Frame { x: 0, y: 0, width: 640, height: 314 }, crop: Some(Area { x: 10, y: 20, width: 300, height: 150 }) };
        assert_eq!(layers[0], solid(1, Art::Block(block)));
        assert_eq!(layers[1], Layer { opacity: 80, ..solid(2, Art::Words(words)) });
        assert_eq!(layers[2], Layer { hidden: true, ..solid(5, Art::Picture(picture)) });
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let layers = three_layers();

        let written = Value::Array(layers.iter().map(layer_as_json).collect());

        assert_eq!(layers_from(&written), Ok(layers));
    }

    #[test]
    fn a_tilt_is_read_and_written_in_whole_degrees_and_an_upright_layer_says_nothing_of_it() {
        let written = r##"{ "kind": "block", "id": 1, "area": { "x": 0, "y": 0, "width": 10, "height": 10 }, "colour": "#ff0000", "tilt": -30 }"##;

        let tilted = layer_from(&serde_json::from_str(written).unwrap()).unwrap();

        assert_eq!(tilted.tilt, -30);
        assert_eq!(layer_as_json(&tilted)["tilt"], -30);
        assert!(layer_as_json(&red_block(1, Frame { x: 0, y: 0, width: 10, height: 10 })).get("tilt").is_none());
        assert!(refusal_of(&written.replace("-30", "200")).contains("\"tilt\" is whole degrees"));
    }

    #[test]
    fn a_plain_block_is_written_without_what_it_does_not_have() {
        let written = layer_as_json(&red_block(4, Frame { x: 1, y: 2, width: 3, height: 4 }));

        assert_eq!(written, json!({ "kind": "block", "id": 4, "shape": "rectangle", "area": { "x": 1, "y": 2, "width": 3, "height": 4 }, "colour": "#ff0000" }));
    }

    #[test]
    fn a_layer_with_something_wrong_says_what_in_plain_words() {
        let block = |rest: &str| format!(r##"{{ "kind": "block", "id": 7, "area": {{ "x": 0, "y": 0, "width": 10, "height": 10 }}, "colour": "#ff0000"{rest} }}"##);

        assert!(refusal_of(r#"{ "kind": "lamp", "id": 7 }"#).contains("\"block\", \"words\" or \"picture\""));
        assert!(refusal_of(r#"{ "kind": "block" }"#).contains("needs a whole number as its \"id\""));
        assert!(refusal_of(&block(r#", "glow": 3"#)).contains("Layer 7: it has no \"glow\""));
        assert!(refusal_of(&block(r#", "shape": "star""#)).contains("\"rectangle\" or \"ellipse\""));
        assert!(refusal_of(&block(r#", "opacity": 140"#)).contains("from 0 to 100"));
        assert!(refusal_of(&block(r##", "edge": { "colour": "#000000", "width": 0 }"##)).contains("from 1 to 64"));
        assert!(refusal_of(&block(r##", "fade": { "colour": "#000000", "from": { "x": 0, "y": 0 } }"##)).contains("two ends"));
        assert!(refusal_of(r##"{ "kind": "block", "id": 7, "area": { "x": 0, "y": 0, "width": 0, "height": 10 }, "colour": "#ff0000" }"##).contains("whole pixels"));
        assert!(refusal_of(r##"{ "kind": "block", "id": 7, "area": { "x": 0, "y": 0, "width": 10, "height": 10 }, "colour": "red" }"##).contains("written like #1e3a28"));
    }

    #[test]
    fn words_and_pictures_must_be_ones_that_can_be_drawn() {
        let words = |text: &str, font: &str| format!(r##"{{ "kind": "words", "id": 1, "text": "{text}", "font": "{font}", "size": 30, "colour": "#ffffff", "place": {{ "x": 0, "y": 0 }} }}"##);
        let picture = |file: &str| format!(r#"{{ "kind": "picture", "id": 1, "file": "{file}", "area": {{ "x": 0, "y": 0, "width": 10, "height": 10 }} }}"#);

        assert!(refusal_of(&words("  ", "Arial")).contains("one line"));
        assert!(refusal_of(&words("TAPE", "Arial' onload='x")).contains("name of a font"));
        assert!(refusal_of(&picture("../plugin.json")).contains("brought into the editor"));
        assert!(layer_from(&serde_json::from_str(&words("TAPE", "Arial")).unwrap()).is_ok());
        assert!(layer_from(&serde_json::from_str(&picture("art-12.png")).unwrap()).is_ok());
    }

    #[test]
    fn two_layers_of_a_page_cannot_share_an_id() {
        let twice = json!([layer_as_json(&red_block(3, Frame { x: 0, y: 0, width: 5, height: 5 })), layer_as_json(&red_block(3, Frame { x: 9, y: 9, width: 5, height: 5 }))]);

        assert_eq!(layers_from(&twice).unwrap_err(), "Two layers of a page share the id 3.");
    }

    #[test]
    fn a_page_cannot_hold_layers_without_end() {
        let many: Vec<Value> = (0..=MOST_LAYERS_ON_A_PAGE as u32).map(|id| layer_as_json(&red_block(id, Frame { x: 0, y: 0, width: 5, height: 5 }))).collect();

        assert!(layers_from(&Value::Array(many)).unwrap_err().contains("60 layers"));
    }

    #[test]
    fn a_colour_is_written_as_six_hex_digits() {
        assert_eq!(Colour::written_as("#1E3a28"), Some(Colour { red: 0x1e, green: 0x3a, blue: 0x28 }));
        assert_eq!(Colour::written_as("#1e3a28").unwrap().written(), "#1e3a28");
        assert!(Colour::written_as("1e3a28").is_none() && Colour::written_as("#1e3a2").is_none() && Colour::written_as("#1e3a2g").is_none());
    }
}
