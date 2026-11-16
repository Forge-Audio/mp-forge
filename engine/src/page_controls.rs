//! Puts a control on a page of the screen's layout, or takes one off, when the musician does so by hand.
//!
//! Which controls a page holds is said by `ui.json`, so that Akai's generator lays them out and gives each
//! its hardware knob. The look only remembers what the musician added and took off. Whenever a look is
//! kept, the difference from the look before it is made to `ui.json` here, which also undoes it again.

use crate::look::{AddedControl, Look, RemovedEntry};
use crate::ordered_json::Json;

const SLOTS_ON_A_PAGE: usize = 16;
const EMPTY_SLOT: &str = "empty";

/// Returns the layout file with every control the newer look added or took off, compared with the
/// look before it. The layout is handed back exactly as it was when the two looks agree about that.
pub fn with_changes_between(layout_file: &str, before: &Look, after: &Look) -> Result<String, String> {
    let changes = changes_between(before, after);
    if changes.is_empty() {
        return Ok(layout_file.to_string());
    }
    let mut layout = Json::parse(layout_file).map_err(|error| format!("ui.json could not be read: {error}"))?;
    for change in &changes {
        let slots = slots_of(&mut layout, change.page()).ok_or_else(|| format!("ui.json has no page called `{}`.", change.page()))?;
        change.make_to(slots)?;
    }
    Ok(layout.written() + "\n")
}

/// One thing to do to a page's list of controls.
#[derive(Debug, PartialEq)]
enum Change<'a> {
    PutOn(&'a str, &'a AddedControl),
    TakeOffAgain(&'a str, &'a AddedControl),
    TakeOff(&'a str, &'a RemovedEntry),
    PutBack(&'a str, &'a RemovedEntry),
}

impl Change<'_> {
    fn page(&self) -> &str {
        match self {
            Change::PutOn(page, _) | Change::TakeOffAgain(page, _) | Change::TakeOff(page, _) | Change::PutBack(page, _) => page,
        }
    }

    fn make_to(&self, slots: &mut Vec<Json>) -> Result<(), String> {
        match self {
            Change::PutOn(page, added) => put_on(slots, added, page),
            Change::TakeOffAgain(_, added) => Ok(empty_slots_showing(slots, &added.control)),
            Change::TakeOff(_, removed) => Ok(take_off(slots, removed)),
            Change::PutBack(page, removed) => put_back(slots, removed, page),
        }
    }
}

/// Things are put back before anything new goes on, so an undone removal gets its own slot again.
fn changes_between<'a>(before: &'a Look, after: &'a Look) -> Vec<Change<'a>> {
    let mut changes = Vec::new();
    for name in pages_named_in(before, after) {
        let (was, is) = (controls_changed_on(before, name), controls_changed_on(after, name));
        changes.extend(was.added.iter().filter(|added| !is.added.contains(added)).map(|added| Change::TakeOffAgain(name, added)));
        changes.extend(was.removed.iter().filter(|removed| !is.removed.contains(removed)).map(|removed| Change::PutBack(name, removed)));
        changes.extend(is.removed.iter().filter(|removed| !was.removed.contains(removed)).map(|removed| Change::TakeOff(name, removed)));
        changes.extend(is.added.iter().filter(|added| !was.added.contains(added)).map(|added| Change::PutOn(name, added)));
    }
    changes
}

/// What a look says the musician added to and took off one page.
struct ControlsChanged<'a> {
    added: &'a [AddedControl],
    removed: &'a [RemovedEntry],
}

fn controls_changed_on<'a>(look: &'a Look, page_name: &str) -> ControlsChanged<'a> {
    match look.of_page(page_name) {
        Some(page) => ControlsChanged { added: &page.added, removed: &page.removed },
        None => ControlsChanged { added: &[], removed: &[] },
    }
}

fn pages_named_in<'a>(before: &'a Look, after: &'a Look) -> Vec<&'a str> {
    let mut names: Vec<&str> = Vec::new();
    for page in after.pages.iter().chain(&before.pages) {
        if !names.contains(&page.page.as_str()) {
            names.push(&page.page);
        }
    }
    names
}

fn slots_of<'a>(layout: &'a mut Json, page_name: &str) -> Option<&'a mut Vec<Json>> {
    let pages = layout.field_mut("pages")?.items_mut()?;
    let position = pages.iter().enumerate().position(|(position, page)| name_of_page(page, position) == page_name)?;
    pages[position].field_mut("controls")?.items_mut()
}

/// A page without a name is known by its number, as on the MPC.
fn name_of_page(page: &Json, position: usize) -> String {
    page.field("name").and_then(Json::as_text).map(str::to_string).unwrap_or_else(|| (position + 1).to_string())
}

fn is_empty(slot: &Json) -> bool {
    slot.as_text() == Some(EMPTY_SLOT) || slot.field("widget").and_then(Json::as_text) == Some(EMPTY_SLOT)
}

fn control_shown_in(slot: &Json) -> Option<&str> {
    slot.as_text().filter(|name| *name != EMPTY_SLOT).or_else(|| slot.field("param").and_then(Json::as_text))
}

/// Puts the entry in the first slot nothing is in. A page whose sixteen slots are all taken has no room.
fn put_in_a_free_slot(slots: &mut Vec<Json>, entry: Json) -> bool {
    match slots.iter().position(is_empty) {
        Some(free) => slots[free] = entry,
        None if slots.len() < SLOTS_ON_A_PAGE => slots.push(entry),
        None => return false,
    }
    true
}

fn put_on(slots: &mut Vec<Json>, added: &AddedControl, page: &str) -> Result<(), String> {
    let already_there = slots.iter().any(|slot| control_shown_in(slot) == Some(&added.control));
    let entry = Json::object([("param", Json::from(added.control.as_str())), ("widget", Json::from(added.widget.as_str()))]);
    if already_there || put_in_a_free_slot(slots, entry) {
        Ok(())
    } else {
        Err(format!("Page `{page}` already holds {SLOTS_ON_A_PAGE} things, so there is no room for `{}`. Take something off it first.", added.control))
    }
}

fn empty_slots_showing(slots: &mut [Json], control: &str) {
    for slot in slots.iter_mut().filter(|slot| control_shown_in(slot) == Some(control)) {
        *slot = Json::from(EMPTY_SLOT);
    }
}

/// Empties the slot, as long as it still holds what the musician took off. Anything else there is the AI's newer work.
fn take_off(slots: &mut [Json], removed: &RemovedEntry) {
    if let Some(slot) = slots.get_mut(removed.slot).filter(|slot| **slot == as_written(removed)) {
        *slot = Json::from(EMPTY_SLOT);
    }
}

/// Puts what was taken off back in its own slot, or in any free one when something else has moved in since.
fn put_back(slots: &mut Vec<Json>, removed: &RemovedEntry, page: &str) -> Result<(), String> {
    while slots.len() <= removed.slot {
        slots.push(Json::from(EMPTY_SLOT));
    }
    if is_empty(&slots[removed.slot]) {
        slots[removed.slot] = as_written(removed);
        return Ok(());
    }
    if put_in_a_free_slot(slots, as_written(removed)) { Ok(()) } else { Err(format!("Page `{page}` has no free slot to put that back in.")) }
}

fn as_written(removed: &RemovedEntry) -> Json {
    Json::parse(&removed.entry.to_string()).unwrap_or(Json::Null)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::look::look_from;

    const LAYOUT: &str = r#"{ "kit": "cyan", "pages": [
        { "name": "MAIN", "controls": [{ "param": "Time", "widget": "knob" }, "empty", "Mix"] },
        { "controls": [] }
    ] }"#;

    fn look_with(page: &str, rest: Value) -> Look {
        let mut written = json!({ "page": page });
        written.as_object_mut().unwrap().extend(rest.as_object().unwrap().clone());
        look_from(&json!({ "pages": [written] })).unwrap()
    }

    fn adding(page: &str, control: &str) -> Look {
        look_with(page, json!({ "added": [{ "control": control, "widget": "knob" }] }))
    }

    fn removing(slot: usize, entry: Value) -> Look {
        look_with("MAIN", json!({ "removed": [{ "slot": slot, "entry": entry }] }))
    }

    fn controls_of(layout_file: &str, page: usize) -> Value {
        serde_json::from_str::<Value>(layout_file).unwrap()["pages"][page]["controls"].clone()
    }

    fn changed(before: &Look, after: &Look) -> String {
        with_changes_between(LAYOUT, before, after).unwrap()
    }

    #[test]
    fn looks_that_agree_about_the_controls_leave_the_layout_exactly_as_written() {
        let look = adding("MAIN", "Volume");

        assert_eq!(with_changes_between(LAYOUT, &look, &look).unwrap(), LAYOUT);
        assert_eq!(with_changes_between(LAYOUT, &Look::default(), &Look::default()).unwrap(), LAYOUT);
    }

    #[test]
    fn a_control_put_on_a_page_takes_its_first_free_slot() {
        let on_main = changed(&Look::default(), &adding("MAIN", "Volume"));
        let on_the_second_page = changed(&Look::default(), &adding("2", "Volume"));

        assert_eq!(controls_of(&on_main, 0), json!([{ "param": "Time", "widget": "knob" }, { "param": "Volume", "widget": "knob" }, "Mix"]));
        assert_eq!(controls_of(&on_the_second_page, 1), json!([{ "param": "Volume", "widget": "knob" }]));
        assert_eq!(serde_json::from_str::<Value>(&on_main).unwrap()["kit"], "cyan");
    }

    #[test]
    fn a_control_already_on_the_page_is_not_put_on_twice() {
        assert_eq!(controls_of(&changed(&Look::default(), &adding("MAIN", "Mix")), 0), controls_of(LAYOUT, 0));
    }

    #[test]
    fn undoing_an_added_control_empties_its_slot_again() {
        let with_volume = changed(&Look::default(), &adding("MAIN", "Volume"));

        let undone = with_changes_between(&with_volume, &adding("MAIN", "Volume"), &Look::default()).unwrap();

        assert_eq!(controls_of(&undone, 0), json!([{ "param": "Time", "widget": "knob" }, "empty", "Mix"]));
    }

    #[test]
    fn something_taken_off_a_page_leaves_its_slot_empty_and_can_be_put_back() {
        let without_time = removing(0, json!({ "param": "Time", "widget": "knob" }));

        let taken_off = changed(&Look::default(), &without_time);
        let put_back = with_changes_between(&taken_off, &without_time, &Look::default()).unwrap();

        assert_eq!(controls_of(&taken_off, 0), json!(["empty", "empty", "Mix"]));
        assert_eq!(controls_of(&put_back, 0), controls_of(LAYOUT, 0));
    }

    #[test]
    fn a_slot_that_no_longer_holds_what_was_taken_off_is_left_alone() {
        let stale = removing(2, json!({ "param": "Tone", "widget": "knob" }));

        assert_eq!(controls_of(&changed(&Look::default(), &stale), 0), controls_of(LAYOUT, 0));
    }

    #[test]
    fn what_was_taken_off_goes_somewhere_free_when_its_own_slot_has_been_taken() {
        let without_mix = removing(2, json!("Mix"));

        let put_back = with_changes_between(LAYOUT, &without_mix, &Look::default()).unwrap();

        assert_eq!(controls_of(&put_back, 0), json!([{ "param": "Time", "widget": "knob" }, "Mix", "Mix"]));
    }

    #[test]
    fn a_full_page_or_a_page_the_layout_lacks_is_refused_in_plain_words() {
        let sixteen: Vec<Value> = (0..16).map(|number| json!(format!("Knob {number}"))).collect();
        let full = json!({ "pages": [{ "name": "MAIN", "controls": sixteen }] }).to_string();

        let no_room = with_changes_between(&full, &Look::default(), &adding("MAIN", "Volume")).unwrap_err();
        let no_page = with_changes_between(LAYOUT, &Look::default(), &adding("EXTRA", "Volume")).unwrap_err();

        assert_eq!(no_room, "Page `MAIN` already holds 16 things, so there is no room for `Volume`. Take something off it first.");
        assert_eq!(no_page, "ui.json has no page called `EXTRA`.");
    }
}
