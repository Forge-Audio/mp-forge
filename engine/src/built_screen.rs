//! Finishes the screen file the build made (`TUI.json`).
//!
//! Puts what the musician moved, renamed or laid out by hand where their arrangement says, changing only
//! entries Akai's generator wrote or copies of them. Then adds the artwork behind each page and the
//! pictures, in the shape Akai's reference gives for an `Image`: a still picture, a film strip that loops,
//! or a film strip that follows a control.

use serde_json::Number;

use crate::arranged_screen::{ArrangedItem, ArrangedPage, TALLEST_FILM_STRIP};
use crate::control_parts::{Align, LabelLook, Parts};
use crate::look::{Depth, ItemKind};
use crate::ordered_json::Json;
use crate::pictures::Picture;
use crate::screen_layout::{Area, WHOLE_PAGE, area_of_slot};

/// Where the MPC finds a picture's image, relative to the screen file.
pub const IMAGES_FOLDER: &str = "pictures";

const NO_FILL: &str = "0";
/// A looping picture ignores its control, but the file format still wants one named. The first always exists.
const ANY_CONTROL: u32 = 0;
/// The name a generated control uses for the parameter it is tied to.
const CONTROL_LINK: &str = "Data";
/// The generator names each heading after its slot: `text 0`, `text 1` and so on.
const HEADING_NAME_START: &str = "text ";
/// Everything the workbench adds beside the screen file starts with this, so it can be told apart and cleared.
pub const LOOK_FILE_START: &str = "look-";
/// The moving images of Akai's control kit, which its own drawing code can draw again at any size.
const KIT_STRIPS: [&str; 4] = ["knob_uni.png", "knob_bi.png", "fader_h_uni.png", "fader_h_bi.png"];
const KIT_FRAMES: u32 = 127;
/// The children the generator gives a control that is drawn with one image: what draws it, by name.
const ART_CHILDREN: [&str; 3] = ["knob", "fader", "toggle"];
const FOCUS_CHILD: &str = "focus";
const NAME_CHILD: &str = "name";
const VALUE_CHILD: &str = "value";
const LEFT: &str = "left";
const RIGHT: &str = "right";
const CENTRED: &str = "horizontallyCentred verticallyCentred";
const SOLID: &str = "ff";

/// One of the kit's moving images drawn again at another size, because the MPC never resizes one.
/// Its file name says everything about it, so the drawing can be asked for from the finished screen alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizedArt {
    /// The kit image it redraws, without its ending: `knob_uni`, `fader_h_bi` and so on.
    pub kit_image: String,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
}

impl SizedArt {
    /// As many frames as the kit has, or fewer when that many would stack taller than the MPC can show.
    fn of(kit_file: &str, size: Area) -> Self {
        let frames = (TALLEST_FILM_STRIP / size.height.max(1)).clamp(2, KIT_FRAMES);
        SizedArt { kit_image: kit_file.trim_end_matches(".png").to_string(), width: size.width, height: size.height, frames }
    }

    pub fn file_name(&self) -> String {
        format!("{LOOK_FILE_START}{}-{}x{}-{}.png", self.kit_image, self.width, self.height, self.frames)
    }

    fn named(file_name: &str) -> Option<Self> {
        let described = file_name.strip_prefix(LOOK_FILE_START)?.strip_suffix(".png")?;
        let [kit_image, size, frames] = described.split('-').collect::<Vec<_>>()[..] else { return None };
        let (width, height) = size.split_once('x')?;
        Some(SizedArt { kit_image: kit_image.to_string(), width: width.parse().ok()?, height: height.parse().ok()?, frames: frames.parse().ok()? })
    }
}

/// Every resized kit image a finished screen asks for, each once.
pub fn sized_art_in(screen_file: &str) -> Vec<SizedArt> {
    let mut found: Vec<SizedArt> = screen_file.split('"').filter_map(SizedArt::named).collect();
    found.dedup();
    found.sort_by_key(SizedArt::file_name);
    found.dedup();
    found
}

/// How the MPC is told to show a picture's frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    Still,
    /// Shows the frame that matches the control with this number. A picture that plays on its own follows
    /// the plugin's clock: an MPC does not play a film strip by itself, whatever `animated` says.
    Following(u32),
}

/// A picture together with what the built plugin says about where it goes.
pub struct PlacedPicture<'a> {
    pub picture: &'a Picture,
    /// Which page it is on, counted from zero.
    pub page_position: usize,
    pub playback: Playback,
    /// The pixels it covers, once the musician's arrangement has been applied.
    pub area: Area,
    /// How far back on its page it sits.
    pub depth: Depth,
}

pub fn image_path_of(picture: &Picture) -> String {
    format!("{IMAGES_FOLDER}/{}.png", picture.name)
}

/// Returns the screen file with everything the musician moved, renamed or laid out put the way the arrangement says.
/// Pages they did not touch are left exactly as the generator wrote them.
pub fn with_arrangement(screen_file: &str, pages: &[ArrangedPage]) -> Result<String, String> {
    let mut screen = Json::parse(screen_file).map_err(|error| format!("The built screen file could not be read: {error}"))?;
    for (page_position, page) in pages.iter().enumerate().filter(|(_, page)| page.is_changed()) {
        arrange_page(&mut screen, page_position, page)?;
    }
    Ok(screen.written())
}

fn arrange_page(screen: &mut Json, page_position: usize, page: &ArrangedPage) -> Result<(), String> {
    let generated = definitions_of(screen).map(|definitions| definitions.to_vec()).unwrap_or_default();
    let mut laid_out = Vec::new();
    let parts = parts_of_page(screen, page_position).ok_or_else(|| format!("The built screen has no page {}.", page_position + 1))?;
    for part in parts.iter_mut() {
        let Some(item) = item_that_owns(part, &page.items) else { continue };
        let own_layout = OwnLayout { key: format!("look {}-{}", page_position + 1, item.slot), generated: &generated };
        match item.parts.as_ref().and_then(|parts| lay_out(part, item, parts, &own_layout)) {
            Some(definition) => laid_out.extend(definition),
            None => move_with_its_slot(part, item),
        }
        show_under_its_name(part, item);
    }
    if let Some(definitions) = definitions_of(screen) {
        definitions.extend(laid_out);
    }
    draw_group_boxes(screen, page_position, &page.group_boxes);
    Ok(())
}

/// The components the screen file defines for itself, each a key and what it is made of.
fn definitions_of(screen: &mut Json) -> Option<&mut Vec<Json>> {
    screen.field_mut("pageData")?.field_mut("componentDefinitions")?.field_mut("localComponentDefinitions")?.items_mut()
}

/// The generator puts everything belonging to a control or heading inside its slot, so a part is owned
/// by the item whose slot holds the part's corner. Each part is asked once, before anything has moved.
fn item_that_owns<'a>(part: &Json, items: &'a [ArrangedItem]) -> Option<&'a ArrangedItem> {
    let as_generated = area_of_part(part)?;
    items.iter().find(|item| item.is_changed() && area_of_slot(item.slot).holds(as_generated.x, as_generated.y))
}

// --- Headings ----------------------------------------------------------------------------------

/// Returns the screen file with every heading tied to a control, so that the MPC draws it.
/// Akai's generator ties a heading to nothing, and a real MPC draws no words that are tied to nothing.
/// The first control always exists, and a heading shows the same words whatever that control is doing.
pub fn with_headings_shown(screen_file: &str) -> Result<String, String> {
    let mut screen = Json::parse(screen_file).map_err(|error| format!("The built screen file could not be read: {error}"))?;
    let pages = screen.field_mut("pageData").and_then(|data| data.field_mut("tabs")).and_then(Json::items_mut);
    for page in pages.into_iter().flatten() {
        let parts = page.field_mut("componentDefinition").and_then(|definition| definition.field_mut("componentsData")).and_then(Json::items_mut);
        parts.into_iter().flatten().filter(|part| is_heading_tied_to_nothing(part)).for_each(tie_to_the_first_control);
    }
    Ok(screen.written())
}

fn links_of(part: &Json) -> Option<&[Json]> {
    part.field("handle remapping")?.field("map")?.items()
}

fn is_heading_tied_to_nothing(part: &Json) -> bool {
    let component = part.field("componentData");
    let text_of = |field: &str| component.and_then(|component| component.field(field)).and_then(Json::as_text).unwrap_or_default();
    let slot_number = text_of("name").strip_prefix(HEADING_NAME_START).unwrap_or_default();
    let named_as_a_heading = !slot_number.is_empty() && slot_number.bytes().all(|byte| byte.is_ascii_digit());
    named_as_a_heading && text_of("type") == "Label" && links_of(part).is_some_and(<[Json]>::is_empty)
}

fn tie_to_the_first_control(heading: &mut Json) {
    let link = Json::object([("key", Json::from(CONTROL_LINK)), ("value", Json::from(format!("Parameter {ANY_CONTROL}")))]);
    let links = heading.field_mut("handle remapping").and_then(|remapping| remapping.field_mut("map")).and_then(Json::items_mut);
    if let Some(links) = links {
        links.push(link);
    }
}

fn show_under_its_name(part: &mut Json, item: &ArrangedItem) {
    if let Some(shown_name) = &item.shown_name {
        match item.kind {
            ItemKind::Heading => say(part, shown_name),
            _ => show_control_as(part, shown_name),
        }
    }
}

// --- A control or heading the musician laid out by hand ----------------------------------------

/// What a hand-made layout is built from: the components the generator defined, and the key its own copy goes under.
struct OwnLayout<'a> {
    key: String,
    generated: &'a [Json],
}

/// Lays the part out as the musician arranged it. Returns the component definitions to add for it, or
/// nothing at all when this part is not one that can be laid out, so that it simply moves with its slot.
fn lay_out(part: &mut Json, item: &ArrangedItem, parts: &Parts, own_layout: &OwnLayout) -> Option<Vec<Json>> {
    match item.kind {
        ItemKind::Heading => lay_out_heading(part, item, parts.name.as_ref()?).then(Vec::new),
        _ => lay_out_control(part, item, parts, own_layout).map(|definition| vec![definition]),
    }
}

/// A heading is one line of lettering, so its place and its lettering are set on the heading itself.
fn lay_out_heading(heading: &mut Json, item: &ArrangedItem, label: &LabelLook) -> bool {
    let within_item = Area { x: item.area.x + label.area.x, y: item.area.y + label.area.y, ..label.area };
    let Some(data) = heading.field_mut("componentData").and_then(|component| component.field_mut("data")) else { return false };
    letter(data, label);
    set_bounds(heading, &within_item);
    true
}

/// Gives the control a copy of its generated component with the parts put where the musician laid them,
/// and points the control at the copy. Only a control drawn with one image, a name and a value has such parts.
fn lay_out_control(control: &mut Json, item: &ArrangedItem, parts: &Parts, own_layout: &OwnLayout) -> Option<Json> {
    let generated_key = control.field("componentData")?.field("type")?.as_text()?.to_string();
    let generated = own_layout.generated.iter().find(|definition| definition.field("key").and_then(Json::as_text) == Some(&generated_key))?;
    let mut own = generated.field("value")?.clone();
    let children = own.field_mut("componentsData")?.items_mut()?;
    if !children.iter().all(is_a_part_that_can_be_laid_out) {
        return None;
    }
    children.retain(|child| !is_hidden(child, parts));
    children.iter_mut().for_each(|child| put_child(child, parts, item));
    *control.field_mut("componentData")?.field_mut("type")? = Json::from(own_layout.key.as_str());
    set_bounds(control, &item.area);
    Some(Json::object([("key", Json::from(own_layout.key.as_str())), ("value", own)]))
}

fn name_of_child(child: &Json) -> &str {
    child.field("componentData").and_then(|component| component.field("name")).and_then(Json::as_text).unwrap_or_default()
}

fn is_a_part_that_can_be_laid_out(child: &Json) -> bool {
    let name = name_of_child(child);
    name == FOCUS_CHILD || name == NAME_CHILD || name == VALUE_CHILD || ART_CHILDREN.contains(&name)
}

fn label_for<'a>(child: &Json, parts: &'a Parts) -> Option<&'a LabelLook> {
    match name_of_child(child) {
        NAME_CHILD => parts.name.as_ref(),
        VALUE_CHILD => parts.value.as_ref(),
        _ => None,
    }
}

fn is_hidden(child: &Json, parts: &Parts) -> bool {
    label_for(child, parts).is_some_and(|label| label.hidden)
}

fn put_child(child: &mut Json, parts: &Parts, item: &ArrangedItem) {
    if name_of_child(child) == FOCUS_CHILD {
        return set_bounds(child, &Area { x: 0, y: 0, ..item.area });
    }
    if let Some(label) = label_for(child, parts) {
        set_bounds(child, &label.area);
        if let Some(data) = child.field_mut("componentData").and_then(|component| component.field_mut("data")) {
            letter(data, label);
        }
    } else if let Some(art) = parts.art {
        put_art(child, art);
    }
}

/// Puts the image that draws the control where it was laid. A kit strip is asked for again at its new size.
/// Any other image keeps the size it was made at, because the MPC would only cut it off.
fn put_art(child: &mut Json, art: Area) {
    let as_generated = area_of_part(child).unwrap_or(art);
    let Some(data) = child.field_mut("componentData").and_then(|component| component.field_mut("data")) else { return };
    let strip = data.field("filmStrip").and_then(Json::as_text).map(str::to_string);
    let draws_itself = data.field("knobType").and_then(Json::as_text) == Some("GenericLegacy");
    let size_changed = (art.width, art.height) != (as_generated.width, as_generated.height);
    match strip {
        Some(file) if KIT_STRIPS.contains(&file.as_str()) && size_changed => {
            let sized = SizedArt::of(&file, art);
            replace(data, "filmStrip", Json::from(sized.file_name()));
            replace(data, "numFrames", Json::from(sized.frames));
            set_bounds(child, &art);
        }
        _ if draws_itself => set_bounds(child, &art),
        _ => set_bounds(child, &Area { width: as_generated.width, height: as_generated.height, ..art }),
    }
}

/// Sets how a line of the MPC's own lettering looks.
fn letter(label_data: &mut Json, label: &LabelLook) {
    let Some(style) = label_data.field_mut("textStyle") else { return };
    let justification = match label.align {
        Align::Left => LEFT,
        Align::Right => RIGHT,
        Align::Centre => CENTRED,
    };
    replace(style, "colour", Json::from(format!("{SOLID}{}", label.colour.written().trim_start_matches('#'))));
    replace(style, "justification", Json::from(justification));
    let Some(font) = style.field_mut("font") else { return };
    replace(font, "name", Json::from(label.font.as_str()));
    replace(font, "style", Json::from(if label.bold { "Bold" } else { "Regular" }));
    replace(font, "height", Number::from_f64(f64::from(label.height)).map_or(Json::Null, Json::Number));
}

fn replace(object: &mut Json, key: &str, value: Json) {
    if let Some(field) = object.field_mut(key) {
        *field = value;
    }
}

fn set_bounds(part: &mut Json, area: &Area) {
    if let Some(bounds) = part.field_mut("bounds").and_then(|bounds| bounds.field_mut("bounds")) {
        *bounds = Json::from(written_area(area));
    }
}

/// Keeps the part where it was inside its slot, and puts the slot where the arrangement says.
fn move_with_its_slot(part: &mut Json, item: &ArrangedItem) {
    let (Some(as_generated), slot) = (area_of_part(part), area_of_slot(item.slot)) else { return };
    let moved = as_generated.moved_to(as_generated.x - slot.x + item.area.x, as_generated.y - slot.y + item.area.y);
    set_bounds(part, &moved);
}

/// The MPC shows the new name wherever it would show the parameter's own. Parts tied to no parameter,
/// such as the words on a switch, are left as they are.
fn show_control_as(part: &mut Json, shown_name: &str) {
    let links = part.field_mut("handle remapping").and_then(|remapping| remapping.field_mut("map")).and_then(Json::items_mut);
    for link in links.into_iter().flatten().filter(|link| link.field("key").and_then(Json::as_text) == Some(CONTROL_LINK)) {
        let Some(parameter) = link.field("value").and_then(Json::as_text).map(str::to_string) else { continue };
        if let Some(value) = link.field_mut("value") {
            *value = Json::from(format!("WithName[{shown_name}]/{parameter}"));
        }
    }
}

fn say(heading: &mut Json, words: &str) {
    let text = heading.field_mut("componentData").and_then(|component| component.field_mut("data")).and_then(|data| data.field_mut("text"));
    if let Some(text) = text {
        *text = Json::from(words);
    }
}

fn draw_group_boxes(screen: &mut Json, page_position: usize, group_boxes: &[Area]) {
    let Some(boxes) = page_at(screen, page_position).and_then(|page| page.field_mut("qlinkBoundsData")) else { return };
    if !group_boxes.is_empty() {
        *boxes = Json::List(group_boxes.iter().map(|group_box| Json::from(written_area(group_box))).collect());
    }
}

fn area_of_part(part: &Json) -> Option<Area> {
    let written = part.field("bounds")?.field("bounds")?.as_text()?;
    let numbers: Vec<u32> = written.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
    let [x, y, width, height] = numbers[..] else { return None };
    Some(Area { x, y, width, height })
}

fn written_area(area: &Area) -> String {
    format!("{} {} {} {}", area.x, area.y, area.width, area.height)
}

/// Returns the screen file with every picture added to its page.
pub fn with_pictures(screen_file: &str, placed: &[PlacedPicture]) -> Result<String, String> {
    let mut screen = Json::parse(screen_file).map_err(|error| format!("The built screen file could not be read: {error}"))?;
    let at_depth = |depth: Depth| placed.iter().filter(move |placement| placement.depth == depth);
    for placement in at_depth(Depth::Front) {
        parts_of_page_holding(&mut screen, placement)?.push(screen_part(placement));
    }
    for placement in at_depth(Depth::Behind).rev() {
        let on_page = parts_of_page_holding(&mut screen, placement)?;
        on_page.insert(first_place_over_the_artwork(on_page), screen_part(placement));
    }
    for placement in at_depth(Depth::Back).rev() {
        parts_of_page_holding(&mut screen, placement)?.insert(0, screen_part(placement));
    }
    Ok(screen.written())
}

fn parts_of_page_holding<'a>(screen: &'a mut Json, placed: &PlacedPicture) -> Result<&'a mut Vec<Json>, String> {
    parts_of_page(screen, placed.page_position).ok_or_else(|| format!("The built screen has no page {}.", placed.page_position + 1))
}

/// Where a picture goes to sit behind the controls but over the musician's artwork, which is one image at the back.
fn first_place_over_the_artwork(on_page: &[Json]) -> usize {
    let is_page_art = |part: &Json| name_of_part(part).is_some_and(|name| name.starts_with(LOOK_FILE_START));
    on_page.iter().position(is_page_art).map_or(0, |page_art| page_art + 1)
}

fn name_of_part(part: &Json) -> Option<&str> {
    part.field("componentData")?.field("name")?.as_text()
}

/// The things drawn on one page, from the back to the front.
fn parts_of_page(screen: &mut Json, page_position: usize) -> Option<&mut Vec<Json>> {
    page_at(screen, page_position)?.field_mut("componentDefinition")?.field_mut("componentsData")?.items_mut()
}

fn page_at(screen: &mut Json, page_position: usize) -> Option<&mut Json> {
    screen.field_mut("pageData")?.field_mut("tabs")?.items_mut()?.get_mut(page_position)
}

fn screen_part(placed: &PlacedPicture) -> Json {
    image_part(&placed.picture.name, image_data(placed), &placed.area)
}

fn image_part(name: &str, data: Json, area: &Area) -> Json {
    let component = Json::object([("version", Json::from(1)), ("name", Json::from(name)), ("type", Json::from("Image")), ("data", data)]);
    let nothing_remapped = Json::object([("version", Json::from(1)), ("map", Json::List(Vec::new()))]);
    Json::object([("version", Json::from(2)), ("componentData", component), ("handle remapping", nothing_remapped), ("bounds", bounds_of(area))])
}

fn still_image(image: Json) -> Json {
    Json::object([("version", Json::from(2)), ("imageType", Json::from("Regular")), ("colour", Json::from(NO_FILL)), ("image", image)])
}

/// The picture that holds all the artwork the musician laid behind one page.
pub fn page_art_file_name(page_position: usize) -> String {
    format!("{LOOK_FILE_START}page-{}.png", page_position + 1)
}

/// Returns the screen file with each of these pages' artwork behind everything else on the page.
/// `pages_with_art` are counted from zero.
pub fn with_page_art(screen_file: &str, pages_with_art: &[usize]) -> Result<String, String> {
    let mut screen = Json::parse(screen_file).map_err(|error| format!("The built screen file could not be read: {error}"))?;
    for page_position in pages_with_art {
        let on_page = parts_of_page(&mut screen, *page_position).ok_or_else(|| format!("The built screen has no page {}.", page_position + 1))?;
        let file = page_art_file_name(*page_position);
        on_page.insert(0, image_part(file.trim_end_matches(".png"), still_image(Json::from(file.as_str())), &WHOLE_PAGE));
    }
    Ok(screen.written())
}

fn image_data(placed: &PlacedPicture) -> Json {
    let image = Json::from(image_path_of(placed.picture));
    let film_strip = |control: u32| {
        Json::object([
            ("version", Json::from(2)),
            ("imageType", Json::from("FilmStrip")),
            ("colour", Json::from(NO_FILL)),
            ("image", image.clone()),
            ("numFrames", Json::from(placed.picture.frames)),
            ("handleName", Json::from(format!("Parameter {control}"))),
            ("animated", Json::from(false)),
        ])
    };
    match placed.playback {
        Playback::Following(control) => film_strip(control),
        Playback::Still => still_image(image.clone()),
    }
}

/// A picture is only looked at: it never takes the cursor, and it shows whatever the controls are doing.
fn bounds_of(area: &Area) -> Json {
    Json::object([
        ("version", Json::from(1)),
        ("acceptsHWFocus", Json::from("No")),
        ("showWhenDataModelInvalid", Json::from("Show")),
        ("whenVisible", Json::from("Always")),
        ("boundsType", Json::from("Absolute")),
        ("bounds", Json::from(written_area(area))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arranged_screen::test_support::{control_in_slot, page_of};
    use crate::control_parts::test_support::{knob_over_its_name_and_value, label_over};
    use crate::layers::Colour;
    use crate::pictures::{Motion, Placement};
    use crate::pictures::test_support::{BOTTOM_LEFT_SLOT, looping_picture};
    use crate::screen_layout::generated_group_box;

    /// Two pages as the build writes them, the first with one control on it.
    const BUILT_SCREEN: &str = r#"{"pageData": {"version": 1, "tabs": [
        {"version": 1, "tabName": "MAIN", "componentDefinition": {"version": 2, "componentsData": [{"version": 2, "componentData": {"name": "Time"}}]}},
        {"version": 1, "tabName": "MORE", "componentDefinition": {"version": 2, "componentsData": []}}
    ]}}"#;

    /// One page as Akai's generator writes it: a knob, a switch with a word on each half, and a heading.
    const GENERATED_SCREEN: &str = r#"{"pageData": {"version": 1, "tabs": [{"version": 1, "tabName": "MAIN",
        "componentDefinition": {"version": 2, "componentsData": [
            {"componentData": {"name": "Time", "type": "knobUni", "data": {"handleName": "Data"}},
             "handle remapping": {"version": 1, "map": [{"key": "Data", "value": "Parameter 1"}]}, "bounds": {"bounds": "23 45 278 107"}},
            {"componentData": {"name": "Sync", "type": "radio2", "data": {"handleName": "Data"}},
             "handle remapping": {"version": 1, "map": [{"key": "Data", "value": "Parameter 2"}]}, "bounds": {"bounds": "343 45 278 107"}},
            {"componentData": {"name": "Sync seg 0", "type": "Label", "data": {"type": "Text", "text": "Off"}},
             "handle remapping": {"version": 1, "map": []}, "bounds": {"bounds": "356 99 122 40"}},
            {"componentData": {"name": "Sync seg 1", "type": "Label", "data": {"type": "Text", "text": "On"}},
             "handle remapping": {"version": 1, "map": []}, "bounds": {"bounds": "486 99 122 40"}},
            {"componentData": {"name": "text 2", "type": "Label", "data": {"type": "Text", "text": "TAPE"}},
             "handle remapping": {"version": 1, "map": []}, "bounds": {"bounds": "663 45 278 107"}}
        ]},
        "qlinkBoundsData": ["38 0 248 624", "358 0 248 624", "678 0 248 624", "998 0 248 624"]}]}}"#;

    /// One page with a kit knob, a switch and a heading, and the components the generator defines for them.
    const SCREEN_WITH_DEFINITIONS: &str = r#"{"pageData": {"version": 1,
        "componentDefinitions": {"version": 2, "importFiles": [], "localComponentDefinitions": [
            {"key": "knobUni", "value": {"version": 2, "actions": [], "componentsData": [
                {"componentData": {"name": "focus", "type": "Focus", "data": {}}, "bounds": {"bounds": "0 0 278 107"}},
                {"componentData": {"name": "knob", "type": "Knob", "data": {"version": 2, "knobType": "FilmStrip", "filmStrip": "knob_uni.png", "numFrames": 127, "handleName": "Data"}}, "bounds": {"bounds": "8 8 90 90"}},
                {"componentData": {"name": "name", "type": "Label", "data": {"textStyle": {"font": {"name": "Titillium Web", "style": "Regular", "height": 26.0}, "colour": "ffa3b8a4", "justification": "left", "case": "Original"}, "type": "Name"}}, "bounds": {"bounds": "110 12 156 36"}},
                {"componentData": {"name": "value", "type": "Label", "data": {"textStyle": {"font": {"name": "Titillium Web", "style": "Regular", "height": 30.0}, "colour": "ffffffff", "justification": "left", "case": "Original"}, "type": "Value"}}, "bounds": {"bounds": "110 52 156 42"}}
            ]}},
            {"key": "radio2", "value": {"version": 2, "actions": [], "componentsData": [
                {"componentData": {"name": "focus", "type": "Focus", "data": {}}, "bounds": {"bounds": "0 0 278 107"}},
                {"componentData": {"name": "seg 0", "type": "Button", "data": {}}, "bounds": {"bounds": "15 54 122 40"}}
            ]}}
        ]},
        "tabs": [{"version": 1, "tabName": "MAIN", "componentDefinition": {"version": 2, "componentsData": [
            {"componentData": {"name": "Time", "type": "knobUni", "data": {"handleName": "Data"}},
             "handle remapping": {"version": 1, "map": [{"key": "Data", "value": "Parameter 1"}]}, "bounds": {"bounds": "23 45 278 107"}},
            {"componentData": {"name": "Sync", "type": "radio2", "data": {"handleName": "Data"}},
             "handle remapping": {"version": 1, "map": [{"key": "Data", "value": "Parameter 2"}]}, "bounds": {"bounds": "343 45 278 107"}},
            {"componentData": {"name": "text 2", "type": "Label", "data": {"textStyle": {"font": {"name": "Titillium Web", "style": "Regular", "height": 30.0}, "colour": "ffffffff", "justification": "left", "case": "Original"}, "type": "Text", "text": "TAPE"}},
             "handle remapping": {"version": 1, "map": []}, "bounds": {"bounds": "663 45 278 107"}}
        ]},
        "qlinkBoundsData": ["38 0 248 624", "358 0 248 624", "678 0 248 624", "998 0 248 624"]}]}}"#;

    fn laid_out_as(item: ArrangedItem, parts: Parts) -> ArrangedItem {
        let (width, height) = parts.size().unwrap();
        ArrangedItem { area: Area { width, height, ..item.area }, parts: Some(parts), ..item }
    }

    fn with_definitions(items: Vec<ArrangedItem>) -> Json {
        Json::parse(&with_arrangement(SCREEN_WITH_DEFINITIONS, &[page_of("MAIN", items)]).unwrap()).unwrap()
    }

    fn definition<'a>(screen: &'a Json, key: &str) -> Option<&'a Json> {
        let definitions = screen.field("pageData")?.field("componentDefinitions")?.field("localComponentDefinitions")?.items()?;
        definitions.iter().find(|definition| definition.field("key").and_then(Json::as_text) == Some(key))?.field("value")
    }

    fn child<'a>(definition: &'a Json, name: &str) -> Option<&'a Json> {
        definition.field("componentsData")?.items()?.iter().find(|child| name_of_child(child) == name)
    }

    fn bounds(part: &Json) -> &str {
        part.field("bounds").unwrap().field("bounds").unwrap().as_text().unwrap()
    }

    fn type_of(part: &Json) -> &str {
        part.field("componentData").unwrap().field("type").unwrap().as_text().unwrap()
    }

    #[test]
    fn a_control_laid_out_by_hand_gets_its_own_copy_of_its_component() {
        let time = moved_to(laid_out_as(control_in_slot("Time", 0), knob_over_its_name_and_value()), 500, 300);

        let screen = with_definitions(vec![time, control_in_slot("Sync", 1)]);

        let own = definition(&screen, "look 1-0").unwrap();
        let control = &parts_on_main_page(&screen)[0];
        assert_eq!((type_of(control), bounds(control)), ("look 1-0", "500 300 120 180"));
        assert_eq!((bounds(child(own, "focus").unwrap()), bounds(child(own, "knob").unwrap())), ("0 0 120 180", "0 0 120 120"));
        assert_eq!((bounds(child(own, "name").unwrap()), bounds(child(own, "value").unwrap())), ("0 120 120 30", "0 150 120 30"));
        assert_eq!(bounds(child(definition(&screen, "knobUni").unwrap(), "knob").unwrap()), "8 8 90 90");
    }

    #[test]
    fn a_resized_kit_knob_asks_for_its_image_at_the_new_size() {
        let screen = with_definitions(vec![laid_out_as(control_in_slot("Time", 0), knob_over_its_name_and_value())]);

        let knob = child(definition(&screen, "look 1-0").unwrap(), "knob").unwrap().field("componentData").unwrap().field("data").unwrap();

        assert_eq!(knob.field("filmStrip").unwrap().as_text(), Some("look-knob_uni-120x120-127.png"));
        assert_eq!(knob.field("numFrames"), Some(&Json::from(127)));
        assert_eq!(sized_art_in(&screen.written()), [SizedArt { kit_image: "knob_uni".to_string(), width: 120, height: 120, frames: 127 }]);
    }

    #[test]
    fn a_big_knob_has_fewer_frames_so_its_strip_stays_short_enough_for_the_mpc() {
        let huge = SizedArt::of("knob_bi.png", Area { x: 0, y: 0, width: 300, height: 300 });

        assert_eq!((huge.frames, huge.file_name().as_str()), (53, "look-knob_bi-300x300-53.png"));
        assert_eq!(SizedArt::named(&huge.file_name()), Some(huge));
        assert_eq!(SizedArt::named("knob_uni.png"), None);
    }

    #[test]
    fn a_knob_that_keeps_its_size_keeps_the_kits_own_image() {
        let same_size = Parts { art: Some(Area { x: 15, y: 0, width: 90, height: 90 }), ..knob_over_its_name_and_value() };

        let screen = with_definitions(vec![laid_out_as(control_in_slot("Time", 0), same_size)]);

        let knob = child(definition(&screen, "look 1-0").unwrap(), "knob").unwrap();
        assert_eq!(bounds(knob), "15 0 90 90");
        assert_eq!(knob.field("componentData").unwrap().field("data").unwrap().field("filmStrip").unwrap().as_text(), Some("knob_uni.png"));
        assert!(sized_art_in(&screen.written()).is_empty());
    }

    #[test]
    fn the_name_and_value_are_lettered_as_asked_and_a_hidden_one_is_left_out() {
        let orange_name = LabelLook { font: "Roboto".to_string(), bold: true, height: 40, colour: Colour { red: 255, green: 136, blue: 0 }, align: Align::Centre, ..label_over(Area { x: 0, y: 120, width: 120, height: 44 }) };
        let hidden_value = LabelLook { hidden: true, ..label_over(Area { x: 0, y: 150, width: 120, height: 30 }) };
        let parts = Parts { name: Some(orange_name), value: Some(hidden_value), ..knob_over_its_name_and_value() };

        let screen = with_definitions(vec![laid_out_as(control_in_slot("Time", 0), parts)]);

        let own = definition(&screen, "look 1-0").unwrap();
        let style = child(own, "name").unwrap().field("componentData").unwrap().field("data").unwrap().field("textStyle").unwrap().written();
        let style: String = style.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(style, r#"{ "font": { "name": "Roboto", "style": "Bold", "height": 40.0 }, "colour": "ffff8800", "justification": "horizontallyCentred verticallyCentred", "case": "Original" }"#);
        assert!(child(own, "value").is_none());
        assert_eq!(bounds(child(own, "focus").unwrap()), "0 0 120 164");
    }

    #[test]
    fn a_control_with_other_parts_just_moves_as_one() {
        let sync = moved_to(laid_out_as(control_in_slot("Sync", 1), knob_over_its_name_and_value()), 600, 400);

        let screen = with_definitions(vec![sync]);

        let control = &parts_on_main_page(&screen)[1];
        assert_eq!((type_of(control), bounds(control)), ("radio2", "600 400 278 107"));
        assert!(definition(&screen, "look 1-1").is_none());
    }

    #[test]
    fn a_heading_is_lettered_and_sized_where_it_stands() {
        let big = LabelLook { height: 60, align: Align::Right, ..label_over(Area { x: 0, y: 0, width: 500, height: 70 }) };
        let heading = moved_to(laid_out_as(heading_in_slot("TAPE", 2), Parts { name: Some(big), ..Parts::default() }), 100, 20);

        let screen = with_definitions(vec![heading]);

        let part = &parts_on_main_page(&screen)[2];
        let style = part.field("componentData").unwrap().field("data").unwrap().field("textStyle").unwrap();
        assert_eq!(bounds(part), "100 20 500 70");
        assert_eq!((style.field("justification").unwrap().as_text(), words_of(part)), (Some("right"), Some("TAPE")));
        assert_eq!(style.field("font").unwrap().field("height").unwrap().written(), "60.0");
    }

    #[test]
    fn a_heading_is_tied_to_the_first_control_so_the_mpc_draws_it_and_nothing_else_is_touched() {
        let screen = Json::parse(&with_headings_shown(GENERATED_SCREEN).unwrap()).unwrap();

        let parts = parts_on_main_page(&screen);

        assert_eq!(link_of(&parts[4]), Some("Parameter 0"));
        assert_eq!((words_of(&parts[4]), bounds_on_main_page(&screen)[4]), (Some("TAPE"), "663 45 278 107"));
        assert_eq!((link_of(&parts[0]), link_of(&parts[1]), link_of(&parts[2]), link_of(&parts[3])), (Some("Parameter 1"), Some("Parameter 2"), None, None));
    }

    #[test]
    fn a_heading_already_tied_to_something_is_left_as_it_is() {
        let once = with_headings_shown(GENERATED_SCREEN).unwrap();

        assert_eq!(with_headings_shown(&once).unwrap(), once);
    }

    #[test]
    fn a_pages_artwork_goes_behind_everything_else_on_it() {
        let screen = Json::parse(&with_page_art(BUILT_SCREEN, &[0]).unwrap()).unwrap();

        let behind = &parts_on_main_page(&screen)[0];

        assert_eq!(names_on_page(&screen, 0), ["look-page-1", "Time"]);
        assert_eq!((bounds(behind), type_of(behind)), ("0 0 1280 628", "Image"));
        assert_eq!(data_of(&screen, 0, 0), r#"{ "version": 2, "imageType": "Regular", "colour": "0", "image": "look-page-1.png" }"#);
        assert_eq!(with_page_art(BUILT_SCREEN, &[5]).unwrap_err(), "The built screen has no page 6.");
    }

    fn moved_to(item: ArrangedItem, x: u32, y: u32) -> ArrangedItem {
        ArrangedItem { area: item.area.moved_to(x, y), ..item }
    }

    fn shown_as(item: ArrangedItem, shown_name: &str) -> ArrangedItem {
        ArrangedItem { shown_name: Some(shown_name.to_string()), ..item }
    }

    fn heading_in_slot(words: &str, slot: usize) -> ArrangedItem {
        ArrangedItem { kind: ItemKind::Heading, ..control_in_slot(words, slot) }
    }

    fn arranged_main_page(items: Vec<ArrangedItem>) -> Json {
        Json::parse(&with_arrangement(GENERATED_SCREEN, &[page_of("MAIN", items)]).unwrap()).unwrap()
    }

    fn parts_on_main_page(screen: &Json) -> &[Json] {
        let pages = screen.field("pageData").unwrap().field("tabs").unwrap().items().unwrap();
        pages[0].field("componentDefinition").unwrap().field("componentsData").unwrap().items().unwrap()
    }

    fn bounds_on_main_page(screen: &Json) -> Vec<&str> {
        parts_on_main_page(screen).iter().map(|part| part.field("bounds").unwrap().field("bounds").unwrap().as_text().unwrap()).collect()
    }

    fn link_of(part: &Json) -> Option<&str> {
        part.field("handle remapping")?.field("map")?.items()?.first()?.field("value")?.as_text()
    }

    fn words_of(part: &Json) -> Option<&str> {
        part.field("componentData")?.field("data")?.field("text")?.as_text()
    }

    #[test]
    fn a_screen_nobody_arranged_is_left_exactly_as_generated() {
        let untouched = page_of("MAIN", vec![control_in_slot("Time", 0), control_in_slot("Sync", 1), heading_in_slot("TAPE", 2)]);

        let screen = with_arrangement(GENERATED_SCREEN, &[untouched]).unwrap();

        assert_eq!(Json::parse(&screen), Json::parse(GENERATED_SCREEN));
    }

    #[test]
    fn a_moved_control_takes_everything_in_its_slot_with_it() {
        let screen = arranged_main_page(vec![control_in_slot("Time", 0), moved_to(control_in_slot("Sync", 1), 600, 400), heading_in_slot("TAPE", 2)]);

        assert_eq!(bounds_on_main_page(&screen), ["23 45 278 107", "600 400 278 107", "613 454 122 40", "743 454 122 40", "663 45 278 107"]);
    }

    #[test]
    fn two_controls_can_take_each_others_places() {
        let swapped = vec![moved_to(control_in_slot("Time", 0), 343, 45), moved_to(control_in_slot("Sync", 1), 23, 45)];

        let screen = arranged_main_page(swapped);

        assert_eq!(bounds_on_main_page(&screen)[..4], ["343 45 278 107", "23 45 278 107", "36 99 122 40", "166 99 122 40"]);
    }

    #[test]
    fn a_renamed_control_is_tied_to_its_parameter_under_the_new_name() {
        let screen = arranged_main_page(vec![shown_as(control_in_slot("Sync", 1), "TEMPO LOCK")]);

        let parts = parts_on_main_page(&screen);

        assert_eq!(link_of(&parts[1]), Some("WithName[TEMPO LOCK]/Parameter 2"));
        assert_eq!((link_of(&parts[0]), words_of(&parts[2]), words_of(&parts[3])), (Some("Parameter 1"), Some("Off"), Some("On")));
        assert_eq!(bounds_on_main_page(&screen)[1], "343 45 278 107");
    }

    #[test]
    fn a_renamed_heading_says_its_new_words() {
        let screen = arranged_main_page(vec![shown_as(moved_to(heading_in_slot("TAPE", 2), 700, 300), "REELS")]);

        let heading = &parts_on_main_page(&screen)[4];

        assert_eq!((words_of(heading), bounds_on_main_page(&screen)[4]), (Some("REELS"), "700 300 278 107"));
    }

    #[test]
    fn the_group_boxes_are_replaced_only_when_the_arrangement_gives_new_ones() {
        let boxes = vec![Area { x: 1, y: 2, width: 300, height: 400 }, generated_group_box(1), generated_group_box(2), generated_group_box(3)];
        let moved = vec![moved_to(control_in_slot("Time", 0), 7, 8)];
        let with_boxes = ArrangedPage { group_boxes: boxes, ..page_of("MAIN", moved.clone()) };
        let boxes_of = |screen: &str| Json::parse(screen).unwrap().field("pageData").unwrap().field("tabs").unwrap().items().unwrap()[0].field("qlinkBoundsData").unwrap().written();

        let redrawn = boxes_of(&with_arrangement(GENERATED_SCREEN, &[with_boxes]).unwrap());
        let kept = boxes_of(&with_arrangement(GENERATED_SCREEN, &[page_of("MAIN", moved)]).unwrap());

        assert!(redrawn.contains("\"1 2 300 400\"") && redrawn.contains("\"358 0 248 624\""), "{redrawn}");
        assert!(kept.contains("\"38 0 248 624\""), "{kept}");
    }

    #[test]
    fn an_arrangement_for_a_page_the_screen_does_not_have_is_refused() {
        let pages = [page_of("MAIN", Vec::new()), page_of("MORE", vec![moved_to(control_in_slot("Time", 0), 7, 8)])];

        assert_eq!(with_arrangement(GENERATED_SCREEN, &pages).unwrap_err(), "The built screen has no page 2.");
    }

    fn placed<'a>(picture: &'a Picture, page_position: usize, playback: Playback) -> PlacedPicture<'a> {
        let depth = if picture.placement == Placement::Backdrop { Depth::Back } else { Depth::Front };
        PlacedPicture { picture, page_position, playback, area: picture.placement.area(), depth }
    }

    fn names_on_page(screen: &Json, page_position: usize) -> Vec<String> {
        let pages = screen.field("pageData").unwrap().field("tabs").unwrap().items().unwrap();
        let parts = pages[page_position].field("componentDefinition").unwrap().field("componentsData").unwrap().items().unwrap();
        parts.iter().map(|part| part.field("componentData").unwrap().field("name").unwrap().as_text().unwrap().to_string()).collect()
    }

    fn data_of(screen: &Json, page_position: usize, part_position: usize) -> String {
        let pages = screen.field("pageData").unwrap().field("tabs").unwrap().items().unwrap();
        let parts = pages[page_position].field("componentDefinition").unwrap().field("componentsData").unwrap().items().unwrap();
        parts[part_position].field("componentData").unwrap().field("data").unwrap().written().split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn a_backdrop_goes_behind_the_controls_and_other_pictures_in_front() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let paper = Picture { placement: Placement::Backdrop, motion: Motion::Still, frames: 1, ..looping_picture("paper", BOTTOM_LEFT_SLOT) };
        let placed = [placed(&reels, 0, Playback::Following(0)), placed(&paper, 0, Playback::Still)];

        let screen = Json::parse(&with_pictures(BUILT_SCREEN, &placed).unwrap()).unwrap();

        assert_eq!(names_on_page(&screen, 0), ["paper", "Time", "reels"]);
        assert_eq!(names_on_page(&screen, 1), Vec::<String>::new());
    }

    #[test]
    fn a_picture_sent_behind_the_controls_sits_over_the_page_art_and_one_sent_right_back_sits_under_it() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let lamp = looping_picture("lamp", BOTTOM_LEFT_SLOT);
        let grain = looping_picture("grain", BOTTOM_LEFT_SLOT);
        let at = |picture, depth| PlacedPicture { depth, ..placed(picture, 0, Playback::Following(0)) };
        let placed = [at(&reels, Depth::Behind), at(&lamp, Depth::Behind), at(&grain, Depth::Back)];
        let with_art = with_page_art(BUILT_SCREEN, &[0]).unwrap();

        let screen = Json::parse(&with_pictures(&with_art, &placed).unwrap()).unwrap();
        let without_art = Json::parse(&with_pictures(BUILT_SCREEN, &placed).unwrap()).unwrap();

        assert_eq!(names_on_page(&screen, 0), ["grain", "look-page-1", "reels", "lamp", "Time"]);
        assert_eq!(names_on_page(&without_art, 0), ["grain", "reels", "lamp", "Time"]);
    }

    #[test]
    fn a_moving_picture_follows_a_control_and_a_still_one_is_a_plain_image() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let needle = Picture { motion: Motion::Follows("Drive".to_string()), ..looping_picture("needle", BOTTOM_LEFT_SLOT) };
        let badge = Picture { motion: Motion::Still, frames: 1, ..looping_picture("badge", BOTTOM_LEFT_SLOT) };
        let clock = 9;
        let placed = [placed(&reels, 1, Playback::Following(clock)), placed(&needle, 1, Playback::Following(7)), placed(&badge, 1, Playback::Still)];

        let screen = Json::parse(&with_pictures(BUILT_SCREEN, &placed).unwrap()).unwrap();

        assert_eq!(
            data_of(&screen, 1, 0),
            r#"{ "version": 2, "imageType": "FilmStrip", "colour": "0", "image": "pictures/reels.png", "numFrames": 4, "handleName": "Parameter 9", "animated": false }"#
        );
        assert_eq!(
            data_of(&screen, 1, 1),
            r#"{ "version": 2, "imageType": "FilmStrip", "colour": "0", "image": "pictures/needle.png", "numFrames": 4, "handleName": "Parameter 7", "animated": false }"#
        );
        assert_eq!(data_of(&screen, 1, 2), r#"{ "version": 2, "imageType": "Regular", "colour": "0", "image": "pictures/badge.png" }"#);
    }

    #[test]
    fn a_picture_sits_exactly_over_its_area_and_never_takes_the_cursor() {
        let bounds = bounds_of(&BOTTOM_LEFT_SLOT.area());

        assert_eq!(bounds.field("bounds").unwrap().as_text(), Some("23 497 278 107"));
        assert_eq!(bounds.field("acceptsHWFocus").unwrap().as_text(), Some("No"));
    }

    #[test]
    fn a_moved_picture_is_added_where_the_arrangement_put_it() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let moved = PlacedPicture { area: reels.placement.area().moved_to(700, 500), ..placed(&reels, 0, Playback::Following(0)) };

        let screen = with_pictures(BUILT_SCREEN, &[moved]).unwrap();

        assert!(screen.contains("\"bounds\": \"700 500 278 107\""), "{screen}");
    }

    #[test]
    fn a_picture_for_a_page_the_screen_does_not_have_is_refused() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);

        let refusal = with_pictures(BUILT_SCREEN, &[placed(&reels, 2, Playback::Following(0))]).unwrap_err();

        assert_eq!(refusal, "The built screen has no page 3.");
    }
}
