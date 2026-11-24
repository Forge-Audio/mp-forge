//! A plugin's screen as it will be shown: each control, heading and picture with the pixels it covers,
//! once what the musician arranged by hand (`look.json`) is laid over the screen's own layout (`ui.json`).
//!
//! The preview, the AI's view of a page and the screen file sent to the MPC all read this one description.

use std::path::Path;

use crate::control_parts::Parts;
use crate::hardware_knobs::{Seat, column_of_knob, knobs_for, own_knob_of};
use crate::layers::Frame;
use crate::look::{Depth, FULLY_SOLID, ItemKind, ItemLook, Look, look_of};
use crate::page_art::frame_of;
use crate::pictures::{Picture, Placement, pictures_of};
use crate::screen_layout::{Area, LayoutPage, SLOTS_PER_SIDE, ScreenLayout, SlotContent, WHOLE_PAGE, area_of_slot};

/// The room left between a group of controls and the box drawn round it, and between a box and the page's edge.
/// Both are what Akai's generator leaves, so a box drawn here sits as one of its own would.
const GROUP_BOX_MARGIN: u32 = 10;
const ROOM_AT_THE_PAGES_EDGE: u32 = 2;
/// The MPC shows nothing of a film strip taller than this, as seen on a real one.
pub const TALLEST_FILM_STRIP: u32 = 16_128;

/// A control or a heading, which the screen's own layout gives one slot of the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrangedItem {
    pub kind: ItemKind,
    pub name: String,
    /// The slot the layout gives it, counted in reading order from zero.
    pub slot: usize,
    /// The hardware knob that turns it: the one beside its slot, unless the musician chose another. A heading has none.
    pub knob: Option<u32>,
    pub area: Area,
    pub shown_name: Option<String>,
    /// How its inside is laid out, when the musician did that by hand.
    pub parts: Option<Parts>,
}

impl ArrangedItem {
    pub fn has_moved(&self) -> bool {
        self.area != area_of_slot(self.slot)
    }

    pub fn is_turned_by_another_knob(&self) -> bool {
        self.knob.is_some_and(|knob| knob != own_knob_of(self.slot))
    }

    pub fn is_changed(&self) -> bool {
        self.has_moved() || self.shown_name.is_some() || self.parts.is_some() || self.is_turned_by_another_knob()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrangedPicture {
    pub name: String,
    pub area: Area,
    pub frames: u32,
    /// How solid it is, out of 100.
    pub opacity: u32,
    pub depth: Depth,
}

impl ArrangedPicture {
    fn strip_height(&self) -> u32 {
        self.area.height.saturating_mul(self.frames)
    }
}

/// One layer of the artwork behind a page's controls, with the pixels it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrangedLayer {
    pub id: u32,
    pub frame: Frame,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrangedPage {
    pub name: String,
    pub items: Vec<ArrangedItem>,
    /// The pictures that show on the page. One the musician took off it is named in `hidden_pictures` instead.
    pub pictures: Vec<ArrangedPicture>,
    pub hidden_pictures: Vec<String>,
    /// The artwork the musician laid behind the controls, from the back to the front.
    pub artwork: Vec<ArrangedLayer>,
    /// The box drawn round the controls of each column of knobs while the hardware knobs are on them.
    /// Empty when no control moved or changed knobs, which leaves the boxes Akai's generator drew.
    pub group_boxes: Vec<Area>,
}

impl ArrangedPage {
    pub fn is_changed(&self) -> bool {
        self.items.iter().any(ArrangedItem::is_changed)
    }

    pub fn has_a_chosen_knob(&self) -> bool {
        self.items.iter().any(ArrangedItem::is_turned_by_another_knob)
    }

    pub fn hides(&self, picture_name: &str) -> bool {
        self.hidden_pictures.iter().any(|hidden| hidden == picture_name)
    }
}

/// The plugin's screen as its files describe it now. Files that cannot be read count as absent.
pub fn arranged_screen_of(plugin: &Path) -> Vec<ArrangedPage> {
    let pictures = pictures_of(plugin).unwrap_or_default();
    let look = look_of(plugin).unwrap_or_default();
    arranged(&ScreenLayout::of_plugin(plugin), &pictures, &look)
}

pub fn arranged(layout: &ScreenLayout, pictures: &[Picture], look: &Look) -> Vec<ArrangedPage> {
    layout.pages.iter().map(|page| arranged_page(page, pictures, look)).collect()
}

/// What stops this arrangement from going on the MPC's screen, in words for the musician.
pub fn problems_with(pages: &[ArrangedPage]) -> Vec<String> {
    pages.iter().flat_map(names_off_the_page).collect()
}

fn names_off_the_page(page: &ArrangedPage) -> Vec<String> {
    let items = page.items.iter().map(|item| (&item.name, item.area));
    let pictures = page.pictures.iter().map(|picture| (&picture.name, picture.area));
    let off_the_page = items.chain(pictures).filter(|(_, area)| !area.is_on_the_page());
    let hanging_off = off_the_page.map(|(name, _)| format!("On page `{}`, `{name}` hangs off the edge of the screen. Move it back onto the page.", page.name));
    hanging_off.chain(page.pictures.iter().filter_map(|picture| too_tall_to_play(&page.name, picture))).collect()
}

fn too_tall_to_play(page_name: &str, picture: &ArrangedPicture) -> Option<String> {
    let fits = picture.strip_height() <= TALLEST_FILM_STRIP;
    let most_frames = TALLEST_FILM_STRIP / picture.area.height.max(1);
    (!fits).then(|| {
        format!(
            "On page `{page_name}`, picture `{}` is {} pixels tall with {} frames, which is more than the MPC can show. At that height it can have {most_frames} frames at most, so make it shorter or give it fewer frames.",
            picture.name, picture.area.height, picture.frames
        )
    })
}

fn arranged_page(page: &LayoutPage, pictures: &[Picture], look: &Look) -> ArrangedPage {
    let in_slots = page.slots().iter().enumerate();
    let in_their_slots: Vec<ArrangedItem> = in_slots.filter_map(|(slot, content)| arranged_item(&page.name, slot, content, look)).collect();
    let items = with_their_knobs(in_their_slots, look, &page.name);
    let on_this_page = pictures.iter().filter(|picture| picture.page == page.name);
    let (hidden, showing): (Vec<&Picture>, Vec<&Picture>) = on_this_page.partition(|picture| look.hides(&page.name, &picture.name));
    ArrangedPage {
        name: page.name.clone(),
        group_boxes: group_boxes_round(&items),
        pictures: showing.into_iter().map(|picture| arranged_picture(picture, look)).collect(),
        hidden_pictures: hidden.into_iter().map(|picture| picture.name.clone()).collect(),
        artwork: look.artwork_of(&page.name).iter().map(|layer| ArrangedLayer { id: layer.id, frame: frame_of(layer), hidden: layer.hidden }).collect(),
        items,
    }
}

fn arranged_item(page: &str, slot: usize, content: &SlotContent, look: &Look) -> Option<ArrangedItem> {
    let (kind, name) = match content {
        SlotContent::Empty => return None,
        SlotContent::Control(name) => (ItemKind::Control, name),
        SlotContent::Heading(words) => (ItemKind::Heading, words),
    };
    let changed = look.of_item(page, kind, name);
    let shown_name = changed.and_then(|item| item.shown_name.clone());
    let parts = changed.and_then(|item| item.parts.clone());
    let area = placed(sized_for(area_of_slot(slot), parts.as_ref()), changed);
    Some(ArrangedItem { kind, name: name.clone(), slot, knob: None, area, shown_name, parts })
}

/// Every control with the hardware knob that turns it, once the musician's own choices are counted.
fn with_their_knobs(mut items: Vec<ArrangedItem>, look: &Look, page: &str) -> Vec<ArrangedItem> {
    let seats: Vec<Seat> = controls_among(&items).map(|control| Seat { control: &control.name, slot: control.slot }).collect();
    let knobs = knobs_for(&seats, look.knobs_chosen_on(page));
    let controls = items.iter_mut().filter(|item| item.kind == ItemKind::Control);
    controls.zip(knobs).for_each(|(control, knob)| control.knob = knob);
    items
}

/// A control laid out by hand is as big as its parts. Any other fills its slot.
fn sized_for(slot: Area, parts: Option<&Parts>) -> Area {
    match parts.and_then(Parts::size) {
        Some((width, height)) => Area { width, height, ..slot },
        None => slot,
    }
}

/// A picture sits over its slots, or fills the page as a backdrop, unless the musician moved or resized it.
fn arranged_picture(picture: &Picture, look: &Look) -> ArrangedPicture {
    let changed = look.of_item(&picture.page, ItemKind::Picture, &picture.name);
    let own_area = picture.placement.area();
    let resized = match changed.and_then(|item| item.size) {
        Some(size) => Area { width: size.width, height: size.height, ..own_area },
        None => own_area,
    };
    let own_depth = if picture.placement == Placement::Backdrop { Depth::Back } else { Depth::Front };
    ArrangedPicture {
        name: picture.name.clone(),
        area: placed(resized, changed),
        frames: picture.frames,
        opacity: changed.and_then(|item| item.opacity).unwrap_or(FULLY_SOLID),
        depth: changed.and_then(|item| item.depth).unwrap_or(own_depth),
    }
}

fn placed(own_area: Area, changed: Option<&ItemLook>) -> Area {
    match changed.and_then(|item| item.place) {
        Some(corner) => own_area.moved_to(corner.x, corner.y),
        None => own_area,
    }
}

fn group_boxes_round(items: &[ArrangedItem]) -> Vec<Area> {
    let the_generated_boxes_still_fit = !controls_among(items).any(|control| control.has_moved() || control.is_turned_by_another_knob());
    if the_generated_boxes_still_fit {
        return Vec::new();
    }
    (0..SLOTS_PER_SIDE as usize).filter_map(|column| group_box_of(column, items)).collect()
}

/// The hardware knobs take one column of knobs at a time, wherever the controls they turn now sit. A column
/// that turns nothing has no box, as in the list Akai's generator writes.
fn group_box_of(column: usize, items: &[ArrangedItem]) -> Option<Area> {
    let in_the_column = controls_among(items).filter(|control| control.knob.map(column_of_knob) == Some(column));
    in_the_column.map(|control| control.area).reduce(around_both).map(with_margin)
}

fn controls_among(items: &[ArrangedItem]) -> impl Iterator<Item = &ArrangedItem> {
    items.iter().filter(|item| item.kind == ItemKind::Control)
}

fn around_both(one: Area, other: Area) -> Area {
    let (x, y) = (one.x.min(other.x), one.y.min(other.y));
    Area { x, y, width: one.right().max(other.right()) - x, height: one.bottom().max(other.bottom()) - y }
}

/// A little room all round, kept just inside the page.
fn with_margin(area: Area) -> Area {
    let (x, y) = (area.x.saturating_sub(GROUP_BOX_MARGIN).max(ROOM_AT_THE_PAGES_EDGE), area.y.saturating_sub(GROUP_BOX_MARGIN).max(ROOM_AT_THE_PAGES_EDGE));
    let right = (area.right() + GROUP_BOX_MARGIN).min(WHOLE_PAGE.width - ROOM_AT_THE_PAGES_EDGE).max(x);
    let bottom = (area.bottom() + GROUP_BOX_MARGIN).min(WHOLE_PAGE.height - ROOM_AT_THE_PAGES_EDGE).max(y);
    Area { x, y, width: right - x, height: bottom - y }
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    pub fn control_in_slot(name: &str, slot: usize) -> ArrangedItem {
        ArrangedItem { kind: ItemKind::Control, name: name.to_string(), slot, knob: Some(own_knob_of(slot)), area: area_of_slot(slot), shown_name: None, parts: None }
    }

    pub fn page_of(name: &str, items: Vec<ArrangedItem>) -> ArrangedPage {
        ArrangedPage { name: name.to_string(), items, pictures: Vec::new(), hidden_pictures: Vec::new(), artwork: Vec::new(), group_boxes: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::control_parts::test_support::knob_over_its_name_and_value;
    use crate::layers::test_support::red_block;
    use crate::look::test_support::{laid_out, look_of_main_page, look_with_artwork, look_with_knobs, moved, renamed};
    use crate::look::{Point, Size};
    use crate::pictures::Motion;
    use crate::pictures::test_support::{BOTTOM_LEFT_SLOT, looping_picture};

    fn tape_layout() -> ScreenLayout {
        ScreenLayout::from_json(&json!({ "pages": [
            { "name": "MAIN", "controls": ["Time", "Mix", "empty", { "widget": "text", "text": "TAPE" }, { "param": "Tone", "widget": "knob" }] },
            { "name": "MORE", "controls": ["Time"] }
        ]}))
    }

    fn item<'a>(pages: &'a [ArrangedPage], page: usize, name: &str) -> &'a ArrangedItem {
        pages[page].items.iter().find(|item| item.name == name).unwrap()
    }

    #[test]
    fn with_nothing_arranged_by_hand_everything_sits_in_its_slot() {
        let pages = arranged(&tape_layout(), &[], &Look::default());

        let names: Vec<&str> = pages[0].items.iter().map(|item| item.name.as_str()).collect();

        assert_eq!(names, ["Time", "Mix", "TAPE", "Tone"]);
        assert_eq!((item(&pages, 0, "TAPE").kind, item(&pages, 0, "TAPE").slot), (ItemKind::Heading, 3));
        assert_eq!(item(&pages, 0, "Tone").area, area_of_slot(4));
        assert!(!pages[0].is_changed() && pages[0].group_boxes.is_empty());
    }

    #[test]
    fn a_moved_control_keeps_its_size_and_its_slot() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Mix", 600, 300)]);

        let pages = arranged(&tape_layout(), &[], &look);

        let mix = item(&pages, 0, "Mix");
        assert_eq!((mix.area, mix.slot), (Area { x: 600, y: 300, width: 278, height: 107 }, 1));
        assert!(mix.has_moved() && pages[0].is_changed());
    }

    #[test]
    fn what_was_arranged_on_one_page_leaves_the_others_alone() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 600, 300), renamed(ItemKind::Control, "Time", "SPEED")]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert_eq!(item(&pages, 1, "Time").area, area_of_slot(0));
        assert!(!pages[1].is_changed());
    }

    #[test]
    fn a_renamed_item_shows_its_new_name_where_it_was() {
        let look = look_of_main_page(vec![renamed(ItemKind::Control, "Tone", "COLOUR"), renamed(ItemKind::Heading, "TAPE", "REELS")]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert_eq!(item(&pages, 0, "Tone").shown_name.as_deref(), Some("COLOUR"));
        assert_eq!(item(&pages, 0, "TAPE").shown_name.as_deref(), Some("REELS"));
        assert!(!item(&pages, 0, "Tone").has_moved() && pages[0].is_changed() && pages[0].group_boxes.is_empty());
    }

    #[test]
    fn edits_for_things_the_screen_no_longer_has_are_ignored() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Wobble", 600, 300), moved(ItemKind::Heading, "Time", 1, 1)]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert!(!pages[0].is_changed());
    }

    #[test]
    fn once_a_control_moves_each_columns_box_is_drawn_round_its_controls() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 500, 400)]);

        let pages = arranged(&tape_layout(), &[], &look);

        let round_what_the_first_column_turns = Area { x: 13, y: 186, width: 775, height: 331 };
        let round_what_the_second_turns = Area { x: 333, y: 35, width: 298, height: 127 };
        assert_eq!(pages[0].group_boxes, [round_what_the_first_column_turns, round_what_the_second_turns], "a column of knobs that turns nothing has no box");
    }

    #[test]
    fn each_control_is_turned_by_the_knob_beside_it_until_the_musician_chooses_another() {
        let as_laid_out = arranged(&tape_layout(), &[], &Look::default());
        let swapped = arranged(&tape_layout(), &[], &look_with_knobs(&[("Time", 14), ("Mix", 13)]));

        let knobs_of = |pages: &[ArrangedPage]| ["Time", "Mix", "TAPE", "Tone"].map(|name| item(pages, 0, name).knob);

        assert_eq!(knobs_of(&as_laid_out), [Some(13), Some(14), None, Some(9)]);
        assert_eq!(knobs_of(&swapped), [Some(14), Some(13), None, Some(9)]);
        assert!(!as_laid_out[0].has_a_chosen_knob() && swapped[0].has_a_chosen_knob() && swapped[0].is_changed());
        assert_eq!(item(&swapped, 1, "Time").knob, Some(13));
    }

    #[test]
    fn once_a_knob_is_chosen_each_box_is_drawn_round_the_controls_its_column_of_knobs_turns() {
        let pages = arranged(&tape_layout(), &[], &look_with_knobs(&[("Time", 14), ("Mix", 13)]));

        let boxes = &pages[0].group_boxes;

        assert_eq!(boxes, &[Area { x: 13, y: 35, width: 618, height: 278 }, Area { x: 13, y: 35, width: 298, height: 127 }]);
    }

    #[test]
    fn a_heading_that_moves_changes_no_box() {
        let look = look_of_main_page(vec![moved(ItemKind::Heading, "TAPE", 500, 400)]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert!(pages[0].is_changed() && pages[0].group_boxes.is_empty());
    }

    #[test]
    fn a_box_stays_just_inside_the_page() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 0, 0), moved(ItemKind::Control, "Tone", 1002, 521)]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert_eq!(pages[0].group_boxes[0], Area { x: 2, y: 2, width: 1276, height: 624 });
    }

    #[test]
    fn a_picture_can_be_moved_and_a_backdrop_fills_the_page_until_it_is_changed() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let paper = Picture { placement: Placement::Backdrop, motion: Motion::Still, frames: 1, ..looping_picture("paper", BOTTOM_LEFT_SLOT) };
        let elsewhere = Picture { page: "MORE".to_string(), ..looping_picture("lamp", BOTTOM_LEFT_SLOT) };
        let look = look_of_main_page(vec![moved(ItemKind::Picture, "reels", 700, 500)]);

        let pages = arranged(&tape_layout(), &[reels, paper, elsewhere], &look);

        assert_eq!(pages[0].pictures, [
            ArrangedPicture { name: "reels".to_string(), area: Area { x: 700, y: 500, width: 278, height: 107 }, frames: 4, opacity: 100, depth: Depth::Front },
            ArrangedPicture { name: "paper".to_string(), area: WHOLE_PAGE, frames: 1, opacity: 100, depth: Depth::Back },
        ]);
        assert_eq!(pages[1].pictures[0].name, "lamp");
    }

    #[test]
    fn a_picture_the_musician_took_off_its_page_is_not_among_those_that_show() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let paper = Picture { placement: Placement::Backdrop, motion: Motion::Still, frames: 1, ..looping_picture("paper", BOTTOM_LEFT_SLOT) };
        let look = look_of_main_page(vec![ItemLook { hidden: true, ..moved(ItemKind::Picture, "reels", 2000, 2000) }]);

        let pages = arranged(&tape_layout(), &[reels, paper], &look);

        assert_eq!(pages[0].pictures.iter().map(|picture| picture.name.as_str()).collect::<Vec<_>>(), ["paper"]);
        assert!(pages[0].hides("reels") && !pages[0].hides("paper"));
        assert_eq!(problems_with(&pages), Vec::<String>::new());
    }

    #[test]
    fn a_picture_is_as_solid_and_as_far_back_as_the_musician_made_it() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let look = look_of_main_page(vec![ItemLook { opacity: Some(40), depth: Some(Depth::Behind), ..ItemLook::of(ItemKind::Picture, "reels") }]);

        let pages = arranged(&tape_layout(), &[reels], &look);

        assert_eq!((pages[0].pictures[0].opacity, pages[0].pictures[0].depth), (40, Depth::Behind));
    }

    #[test]
    fn a_resized_picture_takes_the_size_the_musician_gave_it() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let paper = Picture { placement: Placement::Backdrop, motion: Motion::Still, frames: 1, ..looping_picture("paper", BOTTOM_LEFT_SLOT) };
        let resized = |name: &str, size: Size, place: Option<Point>| ItemLook { size: Some(size), place, ..ItemLook::of(ItemKind::Picture, name) };
        let look = look_of_main_page(vec![
            resized("reels", Size { width: 400, height: 200 }, None),
            resized("paper", Size { width: 640, height: 314 }, Some(Point { x: 600, y: 300 })),
        ]);

        let pages = arranged(&tape_layout(), &[reels, paper], &look);

        assert_eq!(pages[0].pictures[0].area, Area { x: 23, y: 497, width: 400, height: 200 });
        assert_eq!(pages[0].pictures[1].area, Area { x: 600, y: 300, width: 640, height: 314 });
    }

    #[test]
    fn a_picture_whose_frames_stack_taller_than_the_mpc_can_show_is_a_problem() {
        let reels = Picture { frames: 64, ..looping_picture("reels", BOTTOM_LEFT_SLOT) };
        let taller = ItemLook { size: Some(Size { width: 278, height: 300 }), place: Some(Point { x: 23, y: 300 }), ..ItemLook::of(ItemKind::Picture, "reels") };

        let fits = problems_with(&arranged(&tape_layout(), std::slice::from_ref(&reels), &Look::default()));
        let too_tall = problems_with(&arranged(&tape_layout(), &[reels], &look_of_main_page(vec![taller])));

        assert_eq!(fits, Vec::<String>::new());
        assert_eq!(too_tall, ["On page `MAIN`, picture `reels` is 300 pixels tall with 64 frames, which is more than the MPC can show. At that height it can have 53 frames at most, so make it shorter or give it fewer frames."]);
    }

    #[test]
    fn a_control_laid_out_by_hand_is_as_big_as_its_parts() {
        let look = look_of_main_page(vec![ItemLook { place: Some(Point { x: 500, y: 300 }), ..laid_out("Mix", knob_over_its_name_and_value()) }]);

        let pages = arranged(&tape_layout(), &[], &look);

        let mix = item(&pages, 0, "Mix");
        assert_eq!(mix.area, Area { x: 500, y: 300, width: 120, height: 180 });
        assert!(mix.is_changed() && mix.parts.is_some() && item(&pages, 0, "Time").parts.is_none());
    }

    #[test]
    fn the_artwork_behind_a_page_is_listed_with_the_pixels_each_layer_covers() {
        let behind = Frame { x: -20, y: 0, width: 1320, height: 628 };
        let look = look_with_artwork(vec![red_block(4, behind), crate::layers::Layer { hidden: true, ..red_block(9, behind) }]);

        let pages = arranged(&tape_layout(), &[], &look);

        assert_eq!(pages[0].artwork, [ArrangedLayer { id: 4, frame: behind, hidden: false }, ArrangedLayer { id: 9, frame: behind, hidden: true }]);
        assert!(pages[1].artwork.is_empty() && !pages[0].is_changed());
    }

    #[test]
    fn anything_hanging_off_the_edge_of_the_screen_is_a_problem() {
        let reels = looping_picture("reels", BOTTOM_LEFT_SLOT);
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 1100, 45), moved(ItemKind::Picture, "reels", 23, 600)]);

        let problems = problems_with(&arranged(&tape_layout(), &[reels], &look));

        assert_eq!(problems, [
            "On page `MAIN`, `Time` hangs off the edge of the screen. Move it back onto the page.",
            "On page `MAIN`, `reels` hangs off the edge of the screen. Move it back onto the page.",
        ]);
    }

    #[test]
    fn an_arrangement_that_fits_has_no_problems() {
        let look = look_of_main_page(vec![moved(ItemKind::Control, "Time", 1002, 521)]);

        assert_eq!(problems_with(&arranged(&tape_layout(), &[], &look)), Vec::<String>::new());
    }
}
