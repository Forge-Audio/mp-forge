//! Changes the AI asks to make to a plugin's look, the way the musician makes them in the editor.
//!
//! The AI never writes `look.json` itself. It names what to change, one small step at a time, and each
//! step is made to the look as it stands. So everything else done by hand is left as it was, and the
//! result goes through the same checks as a look the musician made.

use serde_json::{Map, Value, json};

use crate::arranged_screen::ArrangedPage;
use crate::layers::{Art, Layer};
use crate::look::{ItemKind, Look, look_from};

const STYLE: &str = "style";
const PUT_BACK: &str = "put_back";
const ADD_ARTWORK: &str = "add_artwork";
const CHANGE_ARTWORK: &str = "change_artwork";
const REMOVE_ARTWORK: &str = "remove_artwork";
const RESTACK_ARTWORK: &str = "restack_artwork";
const KINDS_OF_CHANGE: [&str; 6] = [STYLE, PUT_BACK, ADD_ARTWORK, CHANGE_ARTWORK, REMOVE_ARTWORK, RESTACK_ARTWORK];

/// What can be said about one control, heading or picture. A field given as `null` is taken away again.
const STYLE_FIELDS: [&str; 7] = ["place", "shownName", "size", "parts", "hidden", "opacity", "depth"];
const FIELDS_THAT_NAME_AN_ITEM: [&str; 4] = ["change", "page", "kind", "name"];
const FRONT: &str = "front";
const BACK: &str = "back";

/// The look with every change made to it in turn, or why one of them cannot be made.
/// `screen` is the plugin's screen as it is arranged now, which says what there is to change.
pub fn restyled(look: &Look, screen: &[ArrangedPage], changes: &[Value]) -> Result<Look, String> {
    let mut written = look.as_json();
    for (position, change) in changes.iter().enumerate() {
        make(&mut written, screen, change).map_err(|problem| format!("Change {}: {problem}", position + 1))?;
    }
    look_from(&written)
}

fn make(look: &mut Value, screen: &[ArrangedPage], change: &Value) -> Result<(), String> {
    let fields = change.as_object().ok_or("a change is written between curly brackets.")?;
    let shown = page_shown(screen, text_of(fields, "page")?)?;
    let page = page_written(look, &shown.name);
    match text_of(fields, "change")? {
        STYLE => style(page, shown, fields),
        PUT_BACK => put_back(page, shown, fields),
        ADD_ARTWORK => add_artwork(page, fields),
        CHANGE_ARTWORK => change_artwork(page, fields),
        REMOVE_ARTWORK => remove_artwork(page, fields),
        RESTACK_ARTWORK => restack_artwork(page, fields),
        other => Err(format!("there is no change called `{other}`. There are: {}.", KINDS_OF_CHANGE.join(", "))),
    }
}

fn page_shown<'a>(screen: &'a [ArrangedPage], name: &str) -> Result<&'a ArrangedPage, String> {
    let names: Vec<&str> = screen.iter().map(|page| page.name.as_str()).collect();
    screen.iter().find(|page| page.name == name).ok_or_else(|| format!("the screen has no page called `{name}`. Its pages are: {}.", names.join(", ")))
}

/// What the look says of one page. A page it says nothing of yet is added to it.
fn page_written<'a>(look: &'a mut Value, name: &str) -> &'a mut Value {
    let pages = look["pages"].as_array_mut().expect("a look always lists its pages");
    let position = pages.iter().position(|page| page["page"] == name).unwrap_or_else(|| {
        pages.push(json!({ "page": name, "items": [] }));
        pages.len() - 1
    });
    &mut pages[position]
}

// --- Controls, headings and pictures ----------------------------------------------------------

fn style(page: &mut Value, shown: &ArrangedPage, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &[&FIELDS_THAT_NAME_AN_ITEM[..], &STYLE_FIELDS[..]].concat())?;
    let (kind, name) = item_named(shown, change)?;
    let mut item = taken_from(page, kind, name).unwrap_or_else(|| json!({ "kind": kind.word(), "name": name }));
    for field in STYLE_FIELDS {
        match change.get(field) {
            Some(Value::Null) => drop(item.as_object_mut().and_then(|fields| fields.remove(field))),
            Some(value) => item[field] = value.clone(),
            None => {}
        }
    }
    if says_something(&item) {
        list_of(page, "items").push(item);
    }
    Ok(())
}

fn put_back(page: &mut Value, shown: &ArrangedPage, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &FIELDS_THAT_NAME_AN_ITEM)?;
    let (kind, name) = item_named(shown, change)?;
    taken_from(page, kind, name);
    Ok(())
}

/// The kind and name a change gives, once the page is known to have such a thing.
fn item_named<'a>(shown: &ArrangedPage, change: &'a Map<String, Value>) -> Result<(ItemKind, &'a str), String> {
    let name = text_of(change, "name")?;
    let kind = match text_of(change, "kind")? {
        "control" => ItemKind::Control,
        "heading" => ItemKind::Heading,
        "picture" => ItemKind::Picture,
        other => return Err(format!("`{other}` is not a kind of thing. The kinds are: control, heading, picture.")),
    };
    let there = names_of(shown, kind);
    if there.contains(&name) { Ok((kind, name)) } else { Err(format!("page `{}` has no {} called `{name}`. It has: {}.", shown.name, kind.word(), listed(&there))) }
}

fn names_of(shown: &ArrangedPage, kind: ItemKind) -> Vec<&str> {
    let in_slots = shown.items.iter().filter(|item| item.kind == kind).map(|item| item.name.as_str());
    let pictures = shown.pictures.iter().map(|picture| picture.name.as_str()).chain(shown.hidden_pictures.iter().map(String::as_str));
    if kind == ItemKind::Picture { pictures.collect() } else { in_slots.collect() }
}

fn listed(names: &[&str]) -> String {
    if names.is_empty() { "none".to_string() } else { names.join(", ") }
}

/// Takes what the look says of one thing out of the page, if it says anything.
fn taken_from(page: &mut Value, kind: ItemKind, name: &str) -> Option<Value> {
    let items = list_of(page, "items");
    let position = items.iter().position(|item| item["kind"] == kind.word() && item["name"] == name)?;
    Some(items.remove(position))
}

fn says_something(item: &Value) -> bool {
    STYLE_FIELDS.iter().any(|field| item.get(field).is_some())
}

// --- Artwork ----------------------------------------------------------------------------------

/// New artwork goes in front of the page's other artwork unless it is asked to go behind it all.
fn add_artwork(page: &mut Value, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &["change", "page", "layer", "behind"])?;
    let mut layer = change.get("layer").filter(|layer| layer.is_object()).cloned().ok_or("it needs the new \"layer\", written between curly brackets.")?;
    if layer.get("id").is_some() {
        return Err("a new layer is given its \"id\" by MP Forge, so leave that out.".to_string());
    }
    let artwork = list_of(page, "artwork");
    layer["id"] = json!(artwork.iter().filter_map(|known| known["id"].as_u64()).max().unwrap_or(0) + 1);
    let goes_behind = change.get("behind") == Some(&Value::Bool(true));
    artwork.insert(if goes_behind { 0 } else { artwork.len() }, layer);
    Ok(())
}

fn change_artwork(page: &mut Value, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &["change", "page", "id", "layer"])?;
    let position = position_of_layer(page, change)?;
    let new_fields = change.get("layer").and_then(Value::as_object).ok_or("it needs \"layer\", holding the fields to change.")?;
    if new_fields.contains_key("id") || new_fields.contains_key("kind") {
        return Err("a layer keeps its \"id\" and its \"kind\". Remove it and add another to change those.".to_string());
    }
    let layer = list_of(page, "artwork")[position].as_object_mut().expect("every layer of a look is an object");
    for (field, value) in new_fields {
        if value.is_null() { layer.remove(field); } else { layer.insert(field.clone(), value.clone()); }
    }
    Ok(())
}

fn remove_artwork(page: &mut Value, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &["change", "page", "id"])?;
    let position = position_of_layer(page, change)?;
    list_of(page, "artwork").remove(position);
    Ok(())
}

fn restack_artwork(page: &mut Value, change: &Map<String, Value>) -> Result<(), String> {
    refuse_unknown_fields(change, &["change", "page", "id", "to"])?;
    let position = position_of_layer(page, change)?;
    let end = text_of(change, "to")?;
    let artwork = list_of(page, "artwork");
    let layer = artwork.remove(position);
    match end {
        FRONT => artwork.push(layer),
        BACK => artwork.insert(0, layer),
        _ => return Err(format!("\"to\" is \"{FRONT}\" or \"{BACK}\".")),
    }
    Ok(())
}

fn position_of_layer(page: &mut Value, change: &Map<String, Value>) -> Result<usize, String> {
    let id = change.get("id").and_then(Value::as_u64).ok_or("it needs the \"id\" of the layer, a whole number.")?;
    let artwork = list_of(page, "artwork");
    let ids: Vec<String> = artwork.iter().filter_map(|layer| layer["id"].as_u64()).map(|known| known.to_string()).collect();
    let position = artwork.iter().position(|layer| layer["id"].as_u64() == Some(id));
    position.ok_or_else(|| format!("this page has no artwork with the id {id}. Its artwork has the ids: {}.", if ids.is_empty() { "none".to_string() } else { ids.join(", ") }))
}

// --- Reading a change -------------------------------------------------------------------------

fn text_of<'a>(fields: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    fields.get(name).and_then(Value::as_str).ok_or_else(|| format!("it needs \"{name}\" as text."))
}

fn refuse_unknown_fields(change: &Map<String, Value>, known: &[&str]) -> Result<(), String> {
    match change.keys().find(|field| !known.contains(&field.as_str())) {
        Some(stranger) => Err(format!("this kind of change has no \"{stranger}\". It has: {}.", known.join(", "))),
        None => Ok(()),
    }
}

/// One of a page's lists, which is started when the page does not have it yet.
fn list_of<'a>(page: &'a mut Value, name: &str) -> &'a mut Vec<Value> {
    if !page[name].is_array() {
        page[name] = json!([]);
    }
    page[name].as_array_mut().expect("it was just made a list")
}

// --- Saying where everything is ---------------------------------------------------------------

/// Where everything on the screen sits now, in words, page by page. Artwork is listed from the back to the front.
pub fn described(screen: &[ArrangedPage], look: &Look) -> String {
    screen.iter().map(|page| page_described(page, look)).collect::<Vec<_>>().join("\n\n")
}

fn page_described(page: &ArrangedPage, look: &Look) -> String {
    let items = page.items.iter().map(|item| {
        let shown_as = item.shown_name.as_ref().map(|name| format!(", shown as \"{name}\"")).unwrap_or_default();
        format!("  {} `{}`{shown_as}: at {}, {}, {} by {}", item.kind.word(), item.name, item.area.x, item.area.y, item.area.width, item.area.height)
    });
    let pictures = page.pictures.iter().map(|picture| format!("  picture `{}`: at {}, {}, {} by {}", picture.name, picture.area.x, picture.area.y, picture.area.width, picture.area.height));
    let hidden = page.hidden_pictures.iter().map(|name| format!("  picture `{name}`: hidden"));
    let artwork = look.artwork_of(&page.name).iter().zip(&page.artwork).map(|(layer, arranged)| {
        let frame = arranged.frame;
        format!("  artwork {}: {} at {}, {}, {} by {}{}", layer.id, art_described(layer), frame.x, frame.y, frame.width, frame.height, if layer.hidden { ", hidden" } else { "" })
    });
    let lines: Vec<String> = items.chain(pictures).chain(hidden).chain(artwork).collect();
    format!("Page `{}`\n{}", page.name, if lines.is_empty() { "  (nothing on it)".to_string() } else { lines.join("\n") })
}

fn art_described(layer: &Layer) -> String {
    match &layer.art {
        Art::Block(block) => format!("block of {}", block.colour.written()),
        Art::Words(words) => format!("words \"{}\" in {} at size {}", words.text, words.font, words.size),
        Art::Picture(picture) => format!("the musician's picture {}", picture.file),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arranged_screen::test_support::{control_in_slot, page_of};
    use crate::look::test_support::{look_of_main_page, moved};
    use crate::look::Point;

    fn main_page_with(names: &[&str]) -> Vec<ArrangedPage> {
        vec![page_of("MAIN", names.iter().enumerate().map(|(slot, name)| control_in_slot(name, slot)).collect())]
    }

    fn block(colour: &str) -> Value {
        json!({ "kind": "block", "area": { "x": 0, "y": 0, "width": 1280, "height": 628 }, "colour": colour })
    }

    #[test]
    fn a_control_is_moved_and_renamed_without_touching_what_was_done_to_another() {
        let screen = main_page_with(&["Time", "Mix"]);
        let by_hand = look_of_main_page(vec![moved(ItemKind::Control, "Mix", 700, 300)]);
        let change = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "place": { "x": 40, "y": 60 }, "shownName": "SPEED" });

        let look = restyled(&by_hand, &screen, &[change]).unwrap();

        let time = look.of_item("MAIN", ItemKind::Control, "Time").unwrap();
        assert_eq!((time.place, time.shown_name.as_deref()), (Some(Point { x: 40, y: 60 }), Some("SPEED")));
        assert_eq!(look.of_item("MAIN", ItemKind::Control, "Mix"), Some(&moved(ItemKind::Control, "Mix", 700, 300)));
    }

    #[test]
    fn a_field_given_as_null_is_taken_away_and_put_back_forgets_everything() {
        let screen = main_page_with(&["Time", "Mix"]);
        let by_hand = look_of_main_page(vec![moved(ItemKind::Control, "Time", 40, 60), moved(ItemKind::Control, "Mix", 700, 300)]);
        let unplace = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "place": null });
        let forget = json!({ "change": "put_back", "page": "MAIN", "kind": "control", "name": "Mix" });

        let look = restyled(&by_hand, &screen, &[unplace, forget]).unwrap();

        assert_eq!(look.of_item("MAIN", ItemKind::Control, "Time"), None);
        assert_eq!(look.of_item("MAIN", ItemKind::Control, "Mix"), None);
    }

    #[test]
    fn artwork_is_added_in_front_or_behind_then_changed_restacked_and_removed() {
        let screen = main_page_with(&["Time"]);
        let add_red = json!({ "change": "add_artwork", "page": "MAIN", "layer": block("#ff0000") });
        let add_blue_behind = json!({ "change": "add_artwork", "page": "MAIN", "layer": block("#0000ff"), "behind": true });
        let ids_of = |look: &Look| look.artwork_of("MAIN").iter().map(|layer| layer.id).collect::<Vec<_>>();

        let added = restyled(&Look::default(), &screen, &[add_red, add_blue_behind]).unwrap();
        let recolour = json!({ "change": "change_artwork", "page": "MAIN", "id": 1, "layer": { "colour": "#00ff00", "opacity": 50 } });
        let to_the_back = json!({ "change": "restack_artwork", "page": "MAIN", "id": 1, "to": "back" });
        let changed = restyled(&added, &screen, &[recolour, to_the_back]).unwrap();
        let removed = restyled(&changed, &screen, &[json!({ "change": "remove_artwork", "page": "MAIN", "id": 2 })]).unwrap();

        assert_eq!(ids_of(&added), [2, 1]);
        assert_eq!(ids_of(&changed), [1, 2]);
        assert_eq!(changed.artwork_of("MAIN")[0].opacity, 50);
        assert_eq!(ids_of(&removed), [1]);
    }

    #[test]
    fn a_change_to_something_the_screen_does_not_have_says_what_it_does_have() {
        let screen = main_page_with(&["Time", "Mix"]);
        let wrong_page = json!({ "change": "put_back", "page": "EXTRA", "kind": "control", "name": "Time" });
        let wrong_name = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Speed", "place": { "x": 1, "y": 1 } });
        let wrong_layer = json!({ "change": "remove_artwork", "page": "MAIN", "id": 7 });

        assert_eq!(restyled(&Look::default(), &screen, &[wrong_page]).unwrap_err(), "Change 1: the screen has no page called `EXTRA`. Its pages are: MAIN.");
        assert_eq!(restyled(&Look::default(), &screen, &[wrong_name]).unwrap_err(), "Change 1: page `MAIN` has no control called `Speed`. It has: Time, Mix.");
        assert_eq!(restyled(&Look::default(), &screen, &[wrong_layer]).unwrap_err(), "Change 1: this page has no artwork with the id 7. Its artwork has the ids: none.");
    }

    #[test]
    fn a_change_the_look_cannot_hold_is_refused_whole() {
        let screen = main_page_with(&["Time"]);
        let off_the_page = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "place": { "x": 5000, "y": 0 } });
        let unknown_field = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "colour": "#ffffff" });
        let own_id = json!({ "change": "add_artwork", "page": "MAIN", "layer": { "id": 9, "kind": "block" } });

        assert!(restyled(&Look::default(), &screen, &[off_the_page]).unwrap_err().contains("\"place\""));
        assert!(restyled(&Look::default(), &screen, &[unknown_field]).unwrap_err().contains("has no \"colour\""));
        assert!(restyled(&Look::default(), &screen, &[own_id]).unwrap_err().contains("leave that out"));
    }

    #[test]
    fn the_screen_is_described_with_where_everything_sits() {
        let screen = main_page_with(&["Time"]);

        let words = described(&screen, &Look::default());

        assert_eq!(words, "Page `MAIN`\n  control `Time`: at 23, 45, 278 by 107");
    }
}
