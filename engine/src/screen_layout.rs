//! Where things sit on a plugin's screen: its pages, the four by four grid of slots on each page,
//! and which slots already hold a control.
//!
//! The sizes are the ones Akai's screen generator uses, in pixels of one page of the MPC's screen.

use std::path::Path;

use serde_json::Value;

use crate::library;

/// The file in a plugin's folder that lays out its screen.
pub const SCREEN_LAYOUT: &str = "ui.json";
pub const SLOTS_PER_SIDE: u32 = 4;
pub const WHOLE_PAGE: Area = Area { x: 0, y: 0, width: 1280, height: 628 };

const SLOT_LEFT_EDGES: [u32; 4] = [23, 343, 663, 983];
const SLOT_TOP_EDGES: [u32; 4] = [45, 196, 346, 497];
const SLOT_WIDTH: u32 = 278;
const SLOT_HEIGHT: u32 = 107;
const SLOTS_ON_A_PAGE: usize = (SLOTS_PER_SIDE * SLOTS_PER_SIDE) as usize;
const EMPTY_SLOT: &str = "empty";
/// Two things Akai's generator reads from a layout file: which way the hardware knobs are grouped, and a
/// picture file drawn behind a page.
const KNOB_GROUPS: &str = "qlinkGroups";
const KNOB_GROUPS_FOLLOWED_HERE: &str = "columns";
const BACKDROP_FILE: &str = "background";
const HEADING: &str = "text";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }

    pub fn is_on_the_page(&self) -> bool {
        self.right() <= WHOLE_PAGE.width && self.bottom() <= WHOLE_PAGE.height
    }

    /// Whether the point is inside the area. The right and bottom edges belong to the next area along.
    pub fn holds(&self, x: u32, y: u32) -> bool {
        (self.x..self.right()).contains(&x) && (self.y..self.bottom()).contains(&y)
    }

    pub fn moved_to(&self, x: u32, y: u32) -> Area {
        Area { x, y, ..*self }
    }
}

/// The pixels of one slot, counted in reading order from zero at the top left.
pub fn area_of_slot(position: usize) -> Area {
    let position = position as u32;
    SlotBlock { top_row: position / SLOTS_PER_SIDE, left_column: position % SLOTS_PER_SIDE, rows: 1, columns: 1 }.area()
}

pub fn column_of_slot(position: usize) -> usize {
    position % SLOTS_PER_SIDE as usize
}

/// A rectangle of slots, counted from zero at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotBlock {
    pub top_row: u32,
    pub left_column: u32,
    pub rows: u32,
    pub columns: u32,
}

impl SlotBlock {
    pub fn fits_on_a_page(&self) -> bool {
        let fits = |start: u32, count: u32| count >= 1 && start + count <= SLOTS_PER_SIDE;
        fits(self.top_row, self.rows) && fits(self.left_column, self.columns)
    }

    /// The pixels from the first slot's top left corner to the last slot's bottom right corner.
    pub fn area(&self) -> Area {
        let (last_row, last_column) = (self.top_row + self.rows - 1, self.left_column + self.columns - 1);
        let (x, y) = (SLOT_LEFT_EDGES[self.left_column as usize], SLOT_TOP_EDGES[self.top_row as usize]);
        let right = SLOT_LEFT_EDGES[last_column as usize] + SLOT_WIDTH;
        let bottom = SLOT_TOP_EDGES[last_row as usize] + SLOT_HEIGHT;
        Area { x, y, width: right - x, height: bottom - y }
    }

    /// Every slot in the block as (row, column), counted from zero.
    pub fn slots(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        let rows = self.top_row..self.top_row + self.rows;
        rows.flat_map(move |row| (self.left_column..self.left_column + self.columns).map(move |column| (row, column)))
    }
}

/// What one slot of a page holds, named the way `ui.json` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotContent {
    Empty,
    /// A knob, fader, switch or slider, by the name of the control it shows.
    Control(String),
    /// Fixed words, by what they say.
    Heading(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPage {
    pub name: String,
    slots: Vec<SlotContent>,
}

impl LayoutPage {
    pub fn has_control_at(&self, row: u32, column: u32) -> bool {
        let position = (row * SLOTS_PER_SIDE + column) as usize;
        self.slots.get(position).is_some_and(|content| *content != SlotContent::Empty)
    }

    /// What each slot holds, in reading order. Slots after the last one written are empty.
    pub fn slots(&self) -> &[SlotContent] {
        &self.slots
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScreenLayout {
    pub pages: Vec<LayoutPage>,
}

impl ScreenLayout {
    /// The pages of the plugin's `ui.json`. A plugin without one has no named pages.
    pub fn of_plugin(plugin: &Path) -> Self {
        library::screen_layout(plugin).map(|layout| Self::from_json(&layout)).unwrap_or_default()
    }

    /// The pages a layout file describes. A file that cannot be read describes none.
    pub fn written_in(layout_file: &str) -> Self {
        serde_json::from_str::<Value>(layout_file).map(|layout| Self::from_json(&layout)).unwrap_or_default()
    }

    pub fn from_json(layout: &Value) -> Self {
        let pages = layout["pages"].as_array().map(Vec::as_slice).unwrap_or_default();
        Self { pages: pages.iter().enumerate().map(|(position, page)| page_from_json(position, page)).collect() }
    }

    pub fn position_of(&self, page_name: &str) -> Option<usize> {
        self.pages.iter().position(|page| page.name == page_name)
    }

    pub fn page_names(&self) -> Vec<&str> {
        self.pages.iter().map(|page| page.name.as_str()).collect()
    }
}

/// What a layout file asks of Akai's generator that MP Forge's own drawing of the screen and of the hardware
/// knobs does not follow. The MPC would then show one thing and MP Forge another, so each is said in words.
pub fn asked_for_but_not_followed_here(layout: &Value) -> Vec<String> {
    let pages = layout["pages"].as_array().map(Vec::as_slice).unwrap_or_default();
    let turns_rows_of_knobs = layout[KNOB_GROUPS].as_str().is_some_and(|groups| groups != KNOB_GROUPS_FOLLOWED_HERE);
    let has_a_backdrop_file = pages.iter().any(|page| !page[BACKDROP_FILE].is_null());
    let mut not_followed = Vec::new();
    if turns_rows_of_knobs {
        not_followed.push(format!("ui.json sets `{KNOB_GROUPS}`. Take it out: here the hardware knobs always take one column of controls at a time."));
    }
    if has_a_backdrop_file {
        not_followed.push(format!("A page in ui.json has a `{BACKDROP_FILE}`. Take it out and draw the backdrop as a picture in pictures.json, which MP Forge shows."));
    }
    not_followed
}

/// A page without a name is known by its number, as on the MPC.
fn page_from_json(position: usize, page: &Value) -> LayoutPage {
    let name = page["name"].as_str().map(str::to_string).unwrap_or_else(|| (position + 1).to_string());
    let controls = page["controls"].as_array().map(Vec::as_slice).unwrap_or_default();
    LayoutPage { name, slots: controls.iter().take(SLOTS_ON_A_PAGE).map(content_of).collect() }
}

/// A slot is written as the bare name of a control, the word "empty", or an object saying what to show.
fn content_of(slot: &Value) -> SlotContent {
    let text_of = |field: &str| slot[field].as_str().unwrap_or_default().to_string();
    match (slot.as_str(), slot["widget"].as_str()) {
        (Some(EMPTY_SLOT), _) | (_, Some(EMPTY_SLOT)) => SlotContent::Empty,
        (Some(control), _) => SlotContent::Control(control.to_string()),
        (_, Some(HEADING)) => SlotContent::Heading(text_of("text")),
        _ => SlotContent::Control(text_of("param")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_block_of_slots_covers_from_its_first_corner_to_its_last() {
        let one_slot = SlotBlock { top_row: 0, left_column: 0, rows: 1, columns: 1 };
        let bottom_half = SlotBlock { top_row: 2, left_column: 0, rows: 2, columns: 4 };

        assert_eq!(one_slot.area(), Area { x: 23, y: 45, width: 278, height: 107 });
        assert_eq!(bottom_half.area(), Area { x: 23, y: 346, width: 1238, height: 258 });
    }

    #[test]
    fn a_block_that_runs_off_the_grid_does_not_fit() {
        assert!(SlotBlock { top_row: 3, left_column: 3, rows: 1, columns: 1 }.fits_on_a_page());
        assert!(!SlotBlock { top_row: 3, left_column: 0, rows: 2, columns: 1 }.fits_on_a_page());
        assert!(!SlotBlock { top_row: 0, left_column: 0, rows: 1, columns: 0 }.fits_on_a_page());
    }

    #[test]
    fn pages_know_their_names_and_which_slots_hold_a_control() {
        let layout = ScreenLayout::from_json(&json!({ "pages": [
            { "name": "MAIN", "controls": ["Time", "empty", { "widget": "empty" }, { "widget": "text", "text": "Tape" }, { "param": "Mix", "widget": "fader" }] },
            { "controls": [] }
        ]}));

        let main = &layout.pages[0];

        assert_eq!(layout.page_names(), ["MAIN", "2"]);
        assert_eq!(layout.position_of("2"), Some(1));
        assert!(main.has_control_at(0, 0) && main.has_control_at(0, 3) && main.has_control_at(1, 0));
        assert!(!main.has_control_at(0, 1) && !main.has_control_at(0, 2) && !main.has_control_at(3, 3));
    }

    #[test]
    fn a_page_knows_what_each_slot_holds() {
        let layout = ScreenLayout::from_json(&json!({ "pages": [
            { "name": "MAIN", "controls": ["Time", "empty", { "widget": "text", "text": "Tape" }, { "param": "Mix", "widget": "fader" }] }
        ]}));

        let held = layout.pages[0].slots();

        assert_eq!(held, [
            SlotContent::Control("Time".to_string()),
            SlotContent::Empty,
            SlotContent::Heading("Tape".to_string()),
            SlotContent::Control("Mix".to_string()),
        ]);
    }

    #[test]
    fn a_page_never_has_more_slots_than_the_grid() {
        let seventeen: Vec<Value> = (0..17).map(|number| json!(format!("Knob {number}"))).collect();

        let layout = ScreenLayout::from_json(&json!({ "pages": [{ "name": "MAIN", "controls": seventeen }] }));

        assert_eq!(layout.pages[0].slots().len(), 16);
    }

    #[test]
    fn slots_are_counted_along_each_row_in_turn() {
        assert_eq!(area_of_slot(0), Area { x: 23, y: 45, width: 278, height: 107 });
        assert_eq!(area_of_slot(5), Area { x: 343, y: 196, width: 278, height: 107 });
        assert_eq!((column_of_slot(5), column_of_slot(15)), (1, 3));
    }

    #[test]
    fn what_akais_generator_would_do_but_mp_forge_does_not_show_is_said() {
        let as_mp_forge_shows_it = json!({ "qlinkGroups": "columns", "pages": [{ "name": "MAIN", "controls": ["Time"] }] });
        let knobs_by_the_row = json!({ "qlinkGroups": "rows", "pages": [{ "name": "MAIN", "controls": ["Time"] }] });
        let with_a_backdrop_file = json!({ "pages": [{ "name": "MAIN", "controls": ["Time"], "background": "wood.png" }] });

        assert_eq!(asked_for_but_not_followed_here(&as_mp_forge_shows_it), Vec::<String>::new());
        assert!(asked_for_but_not_followed_here(&knobs_by_the_row)[0].contains("`qlinkGroups`"));
        assert!(asked_for_but_not_followed_here(&with_a_backdrop_file)[0].contains("pictures.json"));
    }

    #[test]
    fn an_area_knows_whether_it_is_on_the_page_and_what_it_holds() {
        let slot = area_of_slot(0);

        assert!(slot.is_on_the_page() && !slot.moved_to(1100, 45).is_on_the_page() && !slot.moved_to(23, 600).is_on_the_page());
        assert!(slot.holds(23, 45) && slot.holds(300, 151) && !slot.holds(301, 45) && !slot.holds(23, 152));
    }
}
