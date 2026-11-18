//! What the musician did by hand to a plugin's screen, as its `look.json` keeps it: where they put a
//! control, a heading or a picture, what they renamed or resized, how they laid out the inside of a
//! control, the artwork they laid behind a page, the controls they added to or took off a page, and
//! the hardware knobs they chose for a page's controls.
//!
//! The file holds only what was changed. Anything it does not mention sits where the screen's own
//! layout, `ui.json`, puts it. Only the workbench writes this file; the AI cannot.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::control_parts::{Parts, parts_as_json, parts_from};
use crate::hardware_knobs::{KNOBS_ON_A_PAGE, KnobChoice};
use crate::layers::{Layer, layer_as_json, layers_from};
use crate::screen_layout::WHOLE_PAGE;

pub const LOOK_FILE: &str = "look.json";

const LONGEST_SHOWN_NAME: usize = 24;
/// Punctuation a shown name may use. Brackets and slashes are left out: the MPC reads them as instructions.
const PUNCTUATION_IN_NAMES: &str = " -+.#&'!?%";
const PAGE_FIELDS: [&str; 6] = ["page", "items", "artwork", "added", "removed", "knobs"];
const ITEM_FIELDS: [&str; 9] = ["kind", "name", "place", "shownName", "size", "parts", "hidden", "opacity", "depth"];
pub const FULLY_SOLID: u32 = 100;
const PLACE_FIELDS: [&str; 2] = ["x", "y"];
const SIZE_FIELDS: [&str; 2] = ["width", "height"];
const ADDED_FIELDS: [&str; 2] = ["control", "widget"];
const REMOVED_FIELDS: [&str; 2] = ["slot", "entry"];
const KNOB_FIELDS: [&str; 2] = ["control", "knob"];
/// The kinds of control the musician can put on a page themselves, as `ui.json` names them.
pub const WIDGETS_TO_ADD: [&str; 4] = ["knob", "fader", "switch", "slider"];
const SMALLEST_PICTURE_SIDE: u64 = 8;
const SLOTS_ON_A_PAGE: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Control,
    Heading,
    Picture,
}

impl ItemKind {
    const ALL: [ItemKind; 3] = [ItemKind::Control, ItemKind::Heading, ItemKind::Picture];

    pub fn word(&self) -> &'static str {
        match self {
            ItemKind::Control => "control",
            ItemKind::Heading => "heading",
            ItemKind::Picture => "picture",
        }
    }

    fn from_word(word: &str) -> Option<ItemKind> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

/// How far back on its page a picture sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Over the controls.
    Front,
    /// Behind the controls, over the musician's artwork.
    Behind,
    /// Behind everything else on the page.
    Back,
}

impl Depth {
    const ALL: [Depth; 3] = [Depth::Front, Depth::Behind, Depth::Back];

    pub fn word(&self) -> &'static str {
        match self {
            Depth::Front => "front",
            Depth::Behind => "behind",
            Depth::Back => "back",
        }
    }
}

/// The top left corner of something, in pixels of one page of the MPC's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

/// How big something is drawn, in pixels of the MPC's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// What was changed about one thing on a page. It is found by its kind and the name `ui.json` or `pictures.json` gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLook {
    pub kind: ItemKind,
    pub name: String,
    pub place: Option<Point>,
    pub shown_name: Option<String>,
    /// The size a picture is drawn at, in place of the size of its slots.
    pub size: Option<Size>,
    /// How the inside of a control or a heading is laid out, in place of the generator's own layout.
    pub parts: Option<Parts>,
    /// A picture the musician took off its page. It is still the plugin's, and can be shown again.
    pub hidden: bool,
    /// How solid a picture is, out of 100, when it is not fully solid.
    pub opacity: Option<u32>,
    /// How far back a picture sits, in place of where `pictures.json` puts it.
    pub depth: Option<Depth>,
}

impl ItemLook {
    pub fn of(kind: ItemKind, name: &str) -> Self {
        ItemLook { kind, name: name.to_string(), place: None, shown_name: None, size: None, parts: None, hidden: false, opacity: None, depth: None }
    }
}

/// A control the musician put on a page themselves. The workbench writes it into the page's free slot in `ui.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedControl {
    pub control: String,
    pub widget: String,
}

/// Something the musician took off a page, kept as `ui.json` had it so that taking it off can be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedEntry {
    /// The slot it was in, counted in reading order from zero.
    pub slot: usize,
    pub entry: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageLook {
    pub page: String,
    pub items: Vec<ItemLook>,
    /// The artwork behind the page's controls, from the back to the front.
    pub artwork: Vec<Layer>,
    pub added: Vec<AddedControl>,
    pub removed: Vec<RemovedEntry>,
    /// The hardware knobs the musician chose for controls of this page, in place of the knob beside each.
    pub knobs: Vec<KnobChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Look {
    pub pages: Vec<PageLook>,
}

impl Look {
    pub fn of_item(&self, page: &str, kind: ItemKind, name: &str) -> Option<&ItemLook> {
        let on_the_page = self.pages.iter().filter(|listed| listed.page == page).flat_map(|listed| &listed.items);
        on_the_page.into_iter().find(|item| item.kind == kind && item.name == name)
    }

    /// Whether the musician took this picture off its page.
    pub fn hides(&self, page: &str, picture: &str) -> bool {
        self.of_item(page, ItemKind::Picture, picture).is_some_and(|item| item.hidden)
    }

    pub fn of_page(&self, page: &str) -> Option<&PageLook> {
        self.pages.iter().find(|listed| listed.page == page)
    }

    /// The hardware knobs the musician chose on one page.
    pub fn knobs_chosen_on(&self, page: &str) -> &[KnobChoice] {
        self.of_page(page).map(|listed| listed.knobs.as_slice()).unwrap_or_default()
    }

    /// The artwork behind one page's controls, from the back to the front.
    pub fn artwork_of(&self, page: &str) -> &[Layer] {
        self.of_page(page).map(|listed| listed.artwork.as_slice()).unwrap_or_default()
    }

    /// Every picture the musician brought in that some layer of this look still shows.
    pub fn art_files(&self) -> Vec<&str> {
        let layers = self.pages.iter().flat_map(|page| &page.artwork);
        layers.filter_map(|layer| match &layer.art {
            crate::layers::Art::Picture(placed) => Some(placed.file.as_str()),
            _ => None,
        }).collect()
    }

    pub fn as_json(&self) -> Value {
        json!({ "pages": self.pages.iter().map(page_as_json).collect::<Vec<_>>() })
    }
}

/// The plugin's look, or why its `look.json` cannot be used. No file means nothing was arranged by hand.
pub fn look_of(plugin: &Path) -> Result<Look, String> {
    match fs::read_to_string(plugin.join(LOOK_FILE)) {
        Ok(written) => look_written_in(&written),
        Err(_) => Ok(Look::default()),
    }
}

pub fn look_written_in(look_file: &str) -> Result<Look, String> {
    let parsed: Value = serde_json::from_str(look_file).map_err(|error| format!("{LOOK_FILE} is not valid JSON: {error}"))?;
    look_from(&parsed)
}

pub fn look_from(look: &Value) -> Result<Look, String> {
    let pages = look["pages"].as_array().ok_or_else(|| format!("{LOOK_FILE} needs a \"pages\" list."))?;
    Ok(Look { pages: pages.iter().map(page_from).collect::<Result<_, _>>()? })
}

pub fn keep_look(plugin: &Path, look: &Look) -> io::Result<()> {
    let written = serde_json::to_string_pretty(&look.as_json()).expect("a look is always plain JSON");
    fs::write(plugin.join(LOOK_FILE), written + "\n")
}

fn page_from(page: &Value) -> Result<PageLook, String> {
    let fields = known_fields(page, &PAGE_FIELDS, "A page")?;
    let name = fields.get("page").and_then(Value::as_str).ok_or("Every page of a look needs its \"page\" name.")?;
    let on_this_page = |problem: String| format!("Page `{name}`: {problem}");
    let list = |field: &str| match fields.get(field) {
        None => Ok(&[][..]),
        Some(listed) => listed.as_array().map(Vec::as_slice).ok_or_else(|| format!("Page `{name}`: its \"{field}\" is a list.")),
    };
    Ok(PageLook {
        page: name.to_string(),
        items: list("items")?.iter().map(item_from).collect::<Result<_, _>>()?,
        artwork: fields.get("artwork").map(layers_from).transpose().map_err(on_this_page)?.unwrap_or_default(),
        added: list("added")?.iter().map(added_from).collect::<Result<_, _>>().map_err(on_this_page)?,
        removed: list("removed")?.iter().map(removed_from).collect::<Result<_, _>>().map_err(on_this_page)?,
        knobs: knobs_from(list("knobs")?).map_err(on_this_page)?,
    })
}

/// Every knob the musician chose on a page. No knob is given twice, and no control is given two knobs.
fn knobs_from(written: &[Value]) -> Result<Vec<KnobChoice>, String> {
    let chosen: Vec<KnobChoice> = written.iter().map(knob_from).collect::<Result<_, _>>()?;
    let given_twice = |choice: &KnobChoice| chosen.iter().filter(|other| other.knob == choice.knob || other.control == choice.control).count() > 1;
    match chosen.iter().find(|choice| given_twice(choice)) {
        Some(choice) => Err(format!("knob {} or control `{}` is listed twice among its \"knobs\".", choice.knob, choice.control)),
        None => Ok(chosen),
    }
}

fn knob_from(choice: &Value) -> Result<KnobChoice, String> {
    let fields = known_fields(choice, &KNOB_FIELDS, "A chosen knob")?;
    let control = fields.get("control").and_then(Value::as_str).filter(|control| !control.trim().is_empty());
    let knob = fields.get("knob").and_then(Value::as_u64).filter(|knob| (1..=u64::from(KNOBS_ON_A_PAGE)).contains(knob));
    match (control, knob) {
        (Some(control), Some(knob)) => Ok(KnobChoice { control: control.to_string(), knob: knob as u32 }),
        _ => Err(format!("a chosen knob has its \"control\" name and a \"knob\" from 1 to {KNOBS_ON_A_PAGE}.")),
    }
}

fn added_from(added: &Value) -> Result<AddedControl, String> {
    let wrong = || format!("a control put on a page has its \"control\" name and a \"widget\": {}.", WIDGETS_TO_ADD.join(", "));
    let fields = known_fields(added, &ADDED_FIELDS, "A control put on a page")?;
    let control = fields.get("control").and_then(Value::as_str).filter(|control| !control.trim().is_empty());
    let widget = fields.get("widget").and_then(Value::as_str).filter(|widget| WIDGETS_TO_ADD.contains(widget));
    match (control, widget) {
        (Some(control), Some(widget)) => Ok(AddedControl { control: control.to_string(), widget: widget.to_string() }),
        _ => Err(wrong()),
    }
}

fn removed_from(removed: &Value) -> Result<RemovedEntry, String> {
    let wrong = || "something taken off a page has the \"slot\" it was in and its \"entry\" from ui.json.".to_string();
    let fields = known_fields(removed, &REMOVED_FIELDS, "Something taken off a page")?;
    let slot = fields.get("slot").and_then(Value::as_u64).filter(|slot| *slot < SLOTS_ON_A_PAGE);
    let entry = fields.get("entry").filter(|entry| entry.is_string() || entry.is_object());
    match (slot, entry) {
        (Some(slot), Some(entry)) => Ok(RemovedEntry { slot: slot as usize, entry: entry.clone() }),
        _ => Err(wrong()),
    }
}

fn item_from(item: &Value) -> Result<ItemLook, String> {
    let fields = known_fields(item, &ITEM_FIELDS, "An item")?;
    let name = fields.get("name").and_then(Value::as_str).ok_or("Every item of a look needs a \"name\".")?;
    let about = |problem: String| format!("`{name}`: {problem}");
    let kind = fields.get("kind").and_then(Value::as_str).and_then(ItemKind::from_word);
    let kind = kind.ok_or_else(|| about("its \"kind\" is \"control\", \"heading\" or \"picture\".".to_string()))?;
    let place = fields.get("place").map(place_from).transpose().map_err(about)?;
    let shown_name = fields.get("shownName").map(shown_name_from).transpose().map_err(about)?;
    let size = fields.get("size").map(size_from).transpose().map_err(about)?;
    let parts = fields.get("parts").map(parts_from).transpose().map_err(about)?;
    let hidden = fields.get("hidden").map(|hidden| hidden_from(hidden, kind)).transpose().map_err(about)?.unwrap_or(false);
    let opacity = fields.get("opacity").map(|opacity| opacity_from(opacity, kind)).transpose().map_err(about)?;
    let depth = fields.get("depth").map(|depth| depth_from(depth, kind)).transpose().map_err(about)?;
    Ok(ItemLook { kind, name: name.to_string(), place, shown_name, size, parts, hidden, opacity, depth })
}

fn opacity_from(opacity: &Value, kind: ItemKind) -> Result<u32, String> {
    let share = opacity.as_u64().filter(|share| *share <= u64::from(FULLY_SOLID));
    match (share, kind) {
        (Some(share), ItemKind::Picture) => Ok(share as u32),
        _ => Err("only a picture has an \"opacity\", which is a whole number from 0 to 100.".to_string()),
    }
}

fn depth_from(depth: &Value, kind: ItemKind) -> Result<Depth, String> {
    let named = depth.as_str().and_then(|word| Depth::ALL.into_iter().find(|depth| depth.word() == word));
    match (named, kind) {
        (Some(depth), ItemKind::Picture) => Ok(depth),
        _ => Err("only a picture has a \"depth\", which is \"front\", \"behind\" or \"back\".".to_string()),
    }
}

/// Only a picture can be hidden. A control or a heading is taken off its page in the layout instead.
fn hidden_from(hidden: &Value, kind: ItemKind) -> Result<bool, String> {
    match (hidden.as_bool(), kind) {
        (Some(hidden), ItemKind::Picture) => Ok(hidden),
        _ => Err("only a picture can be \"hidden\", which is written true or false.".to_string()),
    }
}

fn size_from(size: &Value) -> Result<Size, String> {
    let fields = known_fields(size, &SIZE_FIELDS, "A size")?;
    let pixels = |field: &str, most: u32| fields.get(field).and_then(Value::as_u64).filter(|pixels| (SMALLEST_PICTURE_SIDE..=u64::from(most)).contains(pixels));
    match (pixels("width", WHOLE_PAGE.width), pixels("height", WHOLE_PAGE.height)) {
        (Some(width), Some(height)) => Ok(Size { width: width as u32, height: height as u32 }),
        _ => Err("its \"size\" is whole pixels no bigger than the page, such as {\"width\": 400, \"height\": 200}.".to_string()),
    }
}

fn place_from(place: &Value) -> Result<Point, String> {
    let fields = known_fields(place, &PLACE_FIELDS, "A place")?;
    let whole_pixels = |field: &str| fields.get(field).and_then(Value::as_u64).and_then(|pixels| u32::try_from(pixels).ok());
    match (whole_pixels("x"), whole_pixels("y")) {
        (Some(x), Some(y)) if x < WHOLE_PAGE.width && y < WHOLE_PAGE.height => Ok(Point { x, y }),
        _ => Err("its \"place\" is whole pixels from the page's top left corner, such as {\"x\": 400, \"y\": 120}.".to_string()),
    }
}

fn shown_name_from(shown_name: &Value) -> Result<String, String> {
    let name = shown_name.as_str().ok_or("its \"shownName\" is the words to show.")?;
    match problem_with_shown_name(name) {
        Some(problem) => Err(problem),
        None => Ok(name.to_string()),
    }
}

/// Why these words cannot be shown as a name, in words for the musician. None when they can.
pub fn problem_with_shown_name(name: &str) -> Option<String> {
    let allowed = |letter: char| letter.is_ascii_alphanumeric() || PUNCTUATION_IN_NAMES.contains(letter);
    if name.trim().is_empty() {
        return Some("A name needs at least one letter or number.".to_string());
    }
    if name.chars().count() > LONGEST_SHOWN_NAME {
        return Some(format!("A name can be up to {LONGEST_SHOWN_NAME} characters long."));
    }
    let refused: String = name.chars().filter(|letter| !allowed(*letter)).collect();
    if refused.is_empty() { None } else { Some(format!("A name cannot use these characters: {refused}")) }
}

/// The fields of an object, once none of them is one this file does not know.
fn known_fields<'a>(written: &'a Value, known: &[&str], what: &str) -> Result<&'a Map<String, Value>, String> {
    let fields = written.as_object().ok_or_else(|| format!("{what} of a look is written between curly brackets."))?;
    match fields.keys().find(|key| !known.contains(&key.as_str())) {
        Some(stranger) => Err(format!("{what} of a look has no \"{stranger}\". It has: {}.", known.join(", "))),
        None => Ok(fields),
    }
}

fn page_as_json(page: &PageLook) -> Value {
    let mut written = json!({ "page": page.page, "items": page.items.iter().map(item_as_json).collect::<Vec<_>>() });
    if !page.artwork.is_empty() {
        written["artwork"] = Value::Array(page.artwork.iter().map(layer_as_json).collect());
    }
    if !page.added.is_empty() {
        written["added"] = Value::Array(page.added.iter().map(|added| json!({ "control": added.control, "widget": added.widget })).collect());
    }
    if !page.removed.is_empty() {
        written["removed"] = Value::Array(page.removed.iter().map(|removed| json!({ "slot": removed.slot, "entry": removed.entry })).collect());
    }
    if !page.knobs.is_empty() {
        written["knobs"] = Value::Array(page.knobs.iter().map(|choice| json!({ "control": choice.control, "knob": choice.knob })).collect());
    }
    written
}

fn item_as_json(item: &ItemLook) -> Value {
    let mut written = json!({ "kind": item.kind.word(), "name": item.name });
    if let Some(place) = item.place {
        written["place"] = json!({ "x": place.x, "y": place.y });
    }
    if let Some(shown_name) = &item.shown_name {
        written["shownName"] = json!(shown_name);
    }
    if let Some(size) = item.size {
        written["size"] = json!({ "width": size.width, "height": size.height });
    }
    if let Some(parts) = &item.parts {
        written["parts"] = parts_as_json(parts);
    }
    if item.hidden {
        written["hidden"] = json!(true);
    }
    if let Some(opacity) = item.opacity {
        written["opacity"] = json!(opacity);
    }
    if let Some(depth) = item.depth {
        written["depth"] = json!(depth.word());
    }
    written
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    pub fn moved(kind: ItemKind, name: &str, x: u32, y: u32) -> ItemLook {
        ItemLook { place: Some(Point { x, y }), ..ItemLook::of(kind, name) }
    }

    pub fn renamed(kind: ItemKind, name: &str, shown_name: &str) -> ItemLook {
        ItemLook { shown_name: Some(shown_name.to_string()), ..ItemLook::of(kind, name) }
    }

    pub fn laid_out(name: &str, parts: Parts) -> ItemLook {
        ItemLook { parts: Some(parts), ..ItemLook::of(ItemKind::Control, name) }
    }

    pub fn main_page(items: Vec<ItemLook>) -> PageLook {
        PageLook { page: "MAIN".to_string(), items, ..PageLook::default() }
    }

    pub fn look_of_main_page(items: Vec<ItemLook>) -> Look {
        Look { pages: vec![main_page(items)] }
    }

    pub fn look_with_knobs(knobs: &[(&str, u32)]) -> Look {
        let knobs = knobs.iter().map(|(control, knob)| KnobChoice { control: control.to_string(), knob: *knob }).collect();
        Look { pages: vec![PageLook { knobs, ..main_page(Vec::new()) }] }
    }

    pub fn look_with_artwork(artwork: Vec<Layer>) -> Look {
        Look { pages: vec![PageLook { artwork, ..main_page(Vec::new()) }] }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{look_of_main_page, moved, renamed};
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const MOVED_AND_RENAMED: &str = r#"{ "pages": [{ "page": "MAIN", "items": [
        { "kind": "control", "name": "Time", "place": { "x": 400, "y": 120 }, "shownName": "SPEED" },
        { "kind": "picture", "name": "reels", "place": { "x": 23, "y": 480 } },
        { "kind": "heading", "name": "WOBBLE", "shownName": "Wow & Flutter" }
    ]}]}"#;

    #[test]
    fn a_look_file_is_read_into_what_was_changed() {
        let look = look_written_in(MOVED_AND_RENAMED).unwrap();

        let time = look.of_item("MAIN", ItemKind::Control, "Time").unwrap();

        assert_eq!((time.place, time.shown_name.as_deref()), (Some(Point { x: 400, y: 120 }), Some("SPEED")));
        assert_eq!(look.of_item("MAIN", ItemKind::Picture, "reels"), Some(&moved(ItemKind::Picture, "reels", 23, 480)));
        assert_eq!(look.of_item("MAIN", ItemKind::Heading, "WOBBLE"), Some(&renamed(ItemKind::Heading, "WOBBLE", "Wow & Flutter")));
    }

    #[test]
    fn an_item_is_found_only_by_its_own_page_kind_and_name() {
        let look = look_written_in(MOVED_AND_RENAMED).unwrap();

        assert!(look.of_item("MORE", ItemKind::Control, "Time").is_none());
        assert!(look.of_item("MAIN", ItemKind::Heading, "Time").is_none());
        assert!(look.of_item("MAIN", ItemKind::Control, "time").is_none());
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let look = look_written_in(MOVED_AND_RENAMED).unwrap();

        assert_eq!(look_from(&look.as_json()), Ok(look));
    }

    #[test]
    fn a_plugin_without_a_look_file_has_nothing_arranged_by_hand() {
        let plugin = TemporaryFolder::create("look-none");

        assert_eq!(look_of(&plugin.0), Ok(Look::default()));
    }

    #[test]
    fn a_kept_look_is_the_look_the_plugin_has() {
        let plugin = TemporaryFolder::create("look-kept");
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 400, 120)]);

        keep_look(&plugin.0, &look).unwrap();

        assert_eq!(look_of(&plugin.0), Ok(look));
    }

    #[test]
    fn a_file_that_is_not_a_look_is_refused() {
        assert!(look_written_in("not json").unwrap_err().contains("not valid JSON"));
        assert!(look_written_in("{}").unwrap_err().contains("needs a \"pages\" list"));
        assert!(look_written_in(r#"{ "pages": [{ "page": "MAIN", "items": 3 }] }"#).unwrap_err().contains("its \"items\" is a list"));
        assert_eq!(look_written_in(r#"{ "pages": [{ "page": "MAIN" }] }"#), Ok(look_of_main_page(Vec::new())));
    }

    #[test]
    fn anything_a_look_does_not_know_is_refused() {
        let with = |item: &str| format!(r#"{{ "pages": [{{ "page": "MAIN", "items": [{item}] }}] }}"#);

        let refusals = [
            look_written_in(&with(r#"{ "kind": "control", "name": "Time", "glow": 200 }"#)).unwrap_err(),
            look_written_in(&with(r#"{ "kind": "lamp", "name": "Time" }"#)).unwrap_err(),
            look_written_in(&with(r#"{ "kind": "control", "name": "Time", "place": { "x": -4, "y": 2 } }"#)).unwrap_err(),
            look_written_in(&with(r#"{ "kind": "control", "name": "Time", "place": { "x": 4.5, "y": 2 } }"#)).unwrap_err(),
            look_written_in(&with(r#"{ "kind": "control", "name": "Time", "place": { "x": 4, "y": 2, "z": 1 } }"#)).unwrap_err(),
            look_written_in(&with(r#"{ "kind": "control", "name": "Time", "place": { "x": 1280, "y": 2 } }"#)).unwrap_err(),
        ];

        assert!(refusals[0].contains("has no \"glow\""), "{}", refusals[0]);
        assert!(refusals[1].contains("\"control\", \"heading\" or \"picture\""), "{}", refusals[1]);
        assert!(refusals[2].contains("whole pixels") && refusals[3].contains("whole pixels"), "{refusals:?}");
        assert!(refusals[4].contains("has no \"z\""), "{}", refusals[4]);
        assert!(refusals[5].contains("whole pixels"), "{}", refusals[5]);
    }

    const EVERYTHING_ELSE: &str = r##"{ "pages": [{ "page": "MAIN",
        "items": [
            { "kind": "picture", "name": "reels", "size": { "width": 400, "height": 200 } },
            { "kind": "control", "name": "Time", "parts": { "art": { "x": 0, "y": 0, "width": 120, "height": 120 } } }
        ],
        "artwork": [{ "kind": "picture", "id": 2, "file": "art-7.png", "area": { "x": 0, "y": 0, "width": 1280, "height": 628 } }],
        "added": [{ "control": "Volume", "widget": "knob" }],
        "removed": [{ "slot": 3, "entry": { "param": "Mix", "widget": "fader" } }, { "slot": 4, "entry": "Tone" }]
    }]}"##;

    #[test]
    fn sizes_parts_artwork_and_changes_to_a_pages_controls_are_read_and_written_back() {
        let look = look_written_in(EVERYTHING_ELSE).unwrap();

        let page = look.of_page("MAIN").unwrap();

        assert_eq!(look.of_item("MAIN", ItemKind::Picture, "reels").unwrap().size, Some(Size { width: 400, height: 200 }));
        assert!(look.of_item("MAIN", ItemKind::Control, "Time").unwrap().parts.is_some());
        assert_eq!((look.artwork_of("MAIN").len(), look.artwork_of("MORE").len(), look.art_files()), (1, 0, vec!["art-7.png"]));
        assert_eq!(page.added, [AddedControl { control: "Volume".to_string(), widget: "knob".to_string() }]);
        assert_eq!((page.removed[0].slot, &page.removed[1].entry), (3, &json!("Tone")));
        assert_eq!(look_from(&look.as_json()), Ok(look));
    }

    #[test]
    fn changes_to_a_pages_controls_that_make_no_sense_are_refused() {
        let page_with = |rest: &str| format!(r#"{{ "pages": [{{ "page": "MAIN", {rest} }}] }}"#);

        assert!(look_written_in(&page_with(r#""added": [{ "control": "Volume", "widget": "wheel" }]"#)).unwrap_err().contains("knob, fader, switch, slider"));
        assert!(look_written_in(&page_with(r#""removed": [{ "slot": 16, "entry": "Tone" }]"#)).unwrap_err().contains("the \"slot\" it was in"));
        assert!(look_written_in(&page_with(r#""artwork": [{ "kind": "lamp", "id": 1 }]"#)).unwrap_err().starts_with("Page `MAIN`: Layer 1"));
        assert!(look_written_in(&page_with(r#""items": [{ "kind": "picture", "name": "reels", "size": { "width": 4000, "height": 20 } }]"#)).unwrap_err().contains("no bigger than the page"));
    }

    #[test]
    fn the_knobs_chosen_on_a_page_are_read_and_written_back() {
        let chosen = r#"{ "pages": [{ "page": "MAIN", "items": [], "knobs": [{ "control": "Time", "knob": 14 }, { "control": "Mix", "knob": 13 }] }] }"#;

        let look = look_written_in(chosen).unwrap();

        assert_eq!(look.knobs_chosen_on("MAIN"), [KnobChoice { control: "Time".to_string(), knob: 14 }, KnobChoice { control: "Mix".to_string(), knob: 13 }]);
        assert!(look.knobs_chosen_on("MORE").is_empty());
        assert_eq!(look_from(&look.as_json()), Ok(look));
    }

    #[test]
    fn chosen_knobs_that_make_no_sense_are_refused() {
        let page_with = |knobs: &str| format!(r#"{{ "pages": [{{ "page": "MAIN", "knobs": [{knobs}] }}] }}"#);

        assert!(look_written_in(&page_with(r#"{ "control": "Time", "knob": 17 }"#)).unwrap_err().contains("from 1 to 16"));
        assert!(look_written_in(&page_with(r#"{ "control": "Time", "knob": 0 }"#)).unwrap_err().contains("from 1 to 16"));
        assert!(look_written_in(&page_with(r#"{ "control": " ", "knob": 4 }"#)).unwrap_err().contains("\"control\" name"));
        assert!(look_written_in(&page_with(r#"{ "control": "Time", "knob": 4 }, { "control": "Mix", "knob": 4 }"#)).unwrap_err().contains("listed twice"));
        assert!(look_written_in(&page_with(r#"{ "control": "Time", "knob": 4 }, { "control": "Time", "knob": 5 }"#)).unwrap_err().contains("listed twice"));
    }

    #[test]
    fn a_picture_taken_off_its_page_is_hidden_and_nothing_else_can_be() {
        let hidden = r#"{"pages": [{"page": "MAIN", "items": [{"kind": "picture", "name": "reels", "hidden": true}]}]}"#;
        let hidden_control = r#"{"pages": [{"page": "MAIN", "items": [{"kind": "control", "name": "Mix", "hidden": true}]}]}"#;

        let look = look_written_in(hidden).unwrap();

        assert!(look.hides("MAIN", "reels"));
        assert!(!look.hides("MORE", "reels"));
        assert_eq!(look_from(&look.as_json()), Ok(look));
        assert!(look_written_in(hidden_control).unwrap_err().contains("only a picture can be \"hidden\""));
    }

    #[test]
    fn a_picture_can_be_made_see_through_and_put_further_back_and_nothing_else_can() {
        let styled = r#"{"pages": [{"page": "MAIN", "items": [{"kind": "picture", "name": "reels", "opacity": 40, "depth": "behind"}]}]}"#;
        let too_solid = r#"{"pages": [{"page": "MAIN", "items": [{"kind": "picture", "name": "reels", "opacity": 140}]}]}"#;
        let deep_control = r#"{"pages": [{"page": "MAIN", "items": [{"kind": "control", "name": "Mix", "depth": "back"}]}]}"#;

        let look = look_written_in(styled).unwrap();

        let reels = look.of_item("MAIN", ItemKind::Picture, "reels").unwrap();
        assert_eq!((reels.opacity, reels.depth), (Some(40), Some(Depth::Behind)));
        assert_eq!(look_from(&look.as_json()), Ok(look));
        assert!(look_written_in(too_solid).unwrap_err().contains("\"opacity\""));
        assert!(look_written_in(deep_control).unwrap_err().contains("only a picture has a \"depth\""));
    }

    #[test]
    fn a_shown_name_is_short_plain_words() {
        assert_eq!(problem_with_shown_name("Wow & Flutter 2"), None);
        assert!(problem_with_shown_name("   ").unwrap().contains("at least one"));
        assert!(problem_with_shown_name("a name that goes on far too long").unwrap().contains("up to 24"));
        assert_eq!(problem_with_shown_name("A]/B[").unwrap(), "A name cannot use these characters: ]/[");
    }

    #[test]
    fn a_look_with_a_name_the_mpc_would_misread_is_refused() {
        let written = r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Time", "shownName": "X]/Parameter 0" }] }] }"#;

        assert_eq!(look_written_in(written).unwrap_err(), "`Time`: A name cannot use these characters: ]/");
    }
}
