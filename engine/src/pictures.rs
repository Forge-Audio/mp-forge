//! The pictures a plugin shows on its screen, as its `pictures.json` describes them:
//! what each one is called, which page it is on, where it sits, and whether it moves.
//!
//! The drawing of each picture is a separate file, `pictures/<name>.svg`.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::film_strip::{FrameSize, Timing};
use crate::screen_layout::{Area, ScreenLayout, SlotBlock, WHOLE_PAGE};

pub const PICTURES_FILE: &str = "pictures.json";
pub const DRAWINGS_FOLDER: &str = "pictures";
pub const DRAWING_EXTENSION: &str = "svg";

const MOST_PICTURES: usize = 12;
const MOST_FRAMES: u32 = 64;
const LONGEST_NAME: usize = 32;
/// The MPC has little memory, and every frame of every picture is held in it at once.
const MOST_PIXELS_IN_ONE_PICTURE: u64 = 6_000_000;
const MOST_PIXELS_IN_ALL_PICTURES: u64 = 16_000_000;

const BACKDROP: &str = "backdrop";
const STILL: &str = "still";
const LOOPS: &str = "loops";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Fills the whole page, behind the controls.
    Backdrop,
    /// Covers a block of slots that hold no control.
    Slots(SlotBlock),
}

impl Placement {
    pub fn area(&self) -> Area {
        match self {
            Placement::Backdrop => WHOLE_PAGE,
            Placement::Slots(block) => block.area(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motion {
    Still,
    /// Plays its frames round and round on its own.
    Loops,
    /// Shows the frame that matches where the named control is set.
    Follows(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub name: String,
    pub page: String,
    pub placement: Placement,
    pub motion: Motion,
    pub frames: u32,
}

impl Picture {
    /// Where the AI writes this picture's drawing, relative to the plugin folder.
    pub fn drawing_path(&self) -> String {
        format!("{DRAWINGS_FOLDER}/{}.{DRAWING_EXTENSION}", self.name)
    }

    pub fn frame_size(&self) -> FrameSize {
        let area = self.placement.area();
        FrameSize { width: area.width, height: area.height }
    }

    pub fn timing(&self) -> Timing {
        if self.motion == Motion::Loops { Timing::Looping } else { Timing::EndToEnd }
    }

    fn pixels(&self) -> u64 {
        self.frame_size().pixels() * u64::from(self.frames)
    }

    fn most_frames_that_fit(&self) -> u64 {
        MOST_PIXELS_IN_ONE_PICTURE / self.frame_size().pixels()
    }
}

/// The plugin's pictures, or every reason its `pictures.json` cannot be used. No file means no pictures.
pub fn pictures_of(plugin: &Path) -> Result<Vec<Picture>, Vec<String>> {
    let Ok(written) = fs::read_to_string(plugin.join(PICTURES_FILE)) else { return Ok(Vec::new()) };
    let listed = pictures_written_in(&written)?;
    let problems = problems_with(&listed, &ScreenLayout::of_plugin(plugin));
    if problems.is_empty() { Ok(listed) } else { Err(problems) }
}

pub fn pictures_written_in(pictures_file: &str) -> Result<Vec<Picture>, Vec<String>> {
    let parsed: Value = serde_json::from_str(pictures_file).map_err(|error| vec![format!("{PICTURES_FILE} is not valid JSON: {error}")])?;
    let entries = parsed["pictures"].as_array().ok_or_else(|| vec![format!("{PICTURES_FILE} needs a \"pictures\" list.")])?;
    let (pictures, problems): (Vec<_>, Vec<_>) = entries.iter().map(picture_from).partition(Result::is_ok);
    if problems.is_empty() { Ok(pictures.into_iter().flatten().collect()) } else { Err(problems.into_iter().filter_map(Result::err).collect()) }
}

fn picture_from(entry: &Value) -> Result<Picture, String> {
    let name = name_from(entry)?;
    let about = |problem: String| format!("Picture `{name}`: {problem}");
    let page = entry["page"].as_str().ok_or_else(|| about("it needs a \"page\", the name of the page it is on.".to_string()))?;
    let placement = placement_from(&entry["where"]).map_err(about)?;
    let motion = motion_from(&entry["motion"]).map_err(about)?;
    let frames = frames_from(&entry["frames"], &motion).map_err(about)?;
    Ok(Picture { name: name.clone(), page: page.to_string(), placement, motion, frames })
}

fn name_from(entry: &Value) -> Result<String, String> {
    let name = entry["name"].as_str().ok_or("Every picture needs a \"name\".")?;
    let plain = |letter: char| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-';
    if name.is_empty() || name.len() > LONGEST_NAME || !name.chars().all(plain) {
        return Err(format!("`{name}` cannot be a picture's name. Use up to {LONGEST_NAME} lowercase letters, digits and dashes."));
    }
    Ok(name.to_string())
}

fn placement_from(place: &Value) -> Result<Placement, String> {
    if place.as_str() == Some(BACKDROP) {
        return Ok(Placement::Backdrop);
    }
    let counted_from_one = |field: &str| place[field].as_u64().and_then(|number| u32::try_from(number).ok()).filter(|number| *number >= 1);
    let (Some(row), Some(column)) = (counted_from_one("row"), counted_from_one("column")) else {
        return Err("\"where\" is either \"backdrop\" or a block of slots such as {\"row\": 3, \"column\": 1, \"rows\": 2, \"columns\": 4}.".to_string());
    };
    let block = SlotBlock { top_row: row - 1, left_column: column - 1, rows: counted_from_one("rows").unwrap_or(1), columns: counted_from_one("columns").unwrap_or(1) };
    if block.fits_on_a_page() { Ok(Placement::Slots(block)) } else { Err("its slots run off the page, which has 4 rows and 4 columns.".to_string()) }
}

fn motion_from(motion: &Value) -> Result<Motion, String> {
    match (motion.as_str(), motion["follows"].as_str()) {
        (None, None) if motion.is_null() => Ok(Motion::Still),
        (Some(STILL), _) => Ok(Motion::Still),
        (Some(LOOPS), _) => Ok(Motion::Loops),
        (_, Some(control)) => Ok(Motion::Follows(control.to_string())),
        _ => Err("\"motion\" is \"still\", \"loops\", or {\"follows\": \"the name of a control\"}.".to_string()),
    }
}

fn frames_from(frames: &Value, motion: &Motion) -> Result<u32, String> {
    if *motion == Motion::Still {
        return Ok(1);
    }
    let moving = frames.as_u64().and_then(|count| u32::try_from(count).ok()).filter(|count| (2..=MOST_FRAMES).contains(count));
    moving.ok_or_else(|| format!("a picture that moves needs \"frames\", from 2 to {MOST_FRAMES}."))
}

/// What stops these pictures from going on this screen, in words for whoever wrote them.
pub fn problems_with(pictures: &[Picture], layout: &ScreenLayout) -> Vec<String> {
    let mut problems: Vec<String> = pictures.iter().filter_map(|picture| problem_with(picture, layout)).collect();
    problems.extend(repeated_names(pictures));
    if pictures.len() > MOST_PICTURES {
        problems.push(format!("There are {} pictures; a plugin can have up to {MOST_PICTURES}.", pictures.len()));
    }
    if pictures.iter().map(Picture::pixels).sum::<u64>() > MOST_PIXELS_IN_ALL_PICTURES {
        problems.push("Together the pictures are too much for the MPC's memory. Use fewer frames or smaller pictures.".to_string());
    }
    problems
}

fn problem_with(picture: &Picture, layout: &ScreenLayout) -> Option<String> {
    let about = |problem: String| Some(format!("Picture `{}`: {problem}", picture.name));
    let Some(position) = layout.position_of(&picture.page) else {
        return about(format!("ui.json has no page called `{}`. Its pages are: {}.", picture.page, layout.page_names().join(", ")));
    };
    if let Some((row, column)) = covered_control(picture, layout, position) {
        return about(format!("it covers the control in row {}, column {}. Put it over empty slots, or make it a backdrop.", row + 1, column + 1));
    }
    if picture.pixels() > MOST_PIXELS_IN_ONE_PICTURE {
        return about(format!("at this size it can have at most {} frames.", picture.most_frames_that_fit()));
    }
    None
}

fn covered_control(picture: &Picture, layout: &ScreenLayout, page_position: usize) -> Option<(u32, u32)> {
    let Placement::Slots(block) = &picture.placement else { return None };
    block.slots().find(|(row, column)| layout.pages[page_position].has_control_at(*row, *column))
}

fn repeated_names(pictures: &[Picture]) -> Vec<String> {
    let is_repeat = |(position, picture): (usize, &Picture)| pictures[..position].iter().any(|earlier| earlier.name == picture.name);
    let repeats = pictures.iter().enumerate().filter(|numbered| is_repeat(*numbered));
    repeats.map(|(_, picture)| format!("Two pictures are called `{}`. Give each its own name.", picture.name)).collect()
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    pub fn looping_picture(name: &str, block: SlotBlock) -> Picture {
        Picture { name: name.to_string(), page: "MAIN".to_string(), placement: Placement::Slots(block), motion: Motion::Loops, frames: 4 }
    }

    pub const BOTTOM_LEFT_SLOT: SlotBlock = SlotBlock { top_row: 3, left_column: 0, rows: 1, columns: 1 };
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::test_support::{BOTTOM_LEFT_SLOT, looping_picture};
    use super::*;

    fn layout_with_one_control() -> ScreenLayout {
        ScreenLayout::from_json(&json!({ "pages": [{ "name": "MAIN", "controls": ["Time"] }] }))
    }

    #[test]
    fn a_pictures_file_is_read_into_pictures() {
        let written = r#"{ "pictures": [
            { "name": "reels", "page": "MAIN", "where": { "row": 3, "column": 1, "rows": 2, "columns": 4 }, "motion": "loops", "frames": 12 },
            { "name": "needle", "page": "MAIN", "where": { "row": 2, "column": 4 }, "motion": { "follows": "Drive" }, "frames": 32 },
            { "name": "paper", "page": "MAIN", "where": "backdrop" }
        ]}"#;

        let pictures = pictures_written_in(written).unwrap();

        assert_eq!(pictures[0].placement, Placement::Slots(SlotBlock { top_row: 2, left_column: 0, rows: 2, columns: 4 }));
        assert_eq!((pictures[0].motion.clone(), pictures[0].frames), (Motion::Loops, 12));
        assert_eq!(pictures[1].motion, Motion::Follows("Drive".to_string()));
        assert_eq!(pictures[1].frame_size(), FrameSize { width: 278, height: 107 });
        assert_eq!((pictures[2].placement.clone(), pictures[2].motion.clone(), pictures[2].frames), (Placement::Backdrop, Motion::Still, 1));
        assert_eq!(pictures[2].drawing_path(), "pictures/paper.svg");
    }

    #[test]
    fn every_mistake_in_a_pictures_file_is_reported_by_picture() {
        let written = r#"{ "pictures": [
            { "name": "Reels!", "page": "MAIN", "where": "backdrop" },
            { "name": "a", "where": "backdrop" },
            { "name": "b", "page": "MAIN", "where": { "row": 4, "column": 1, "rows": 2 } },
            { "name": "c", "page": "MAIN", "where": "backdrop", "motion": "loops" },
            { "name": "d", "page": "MAIN", "where": "backdrop", "motion": "spins" }
        ]}"#;

        let problems = pictures_written_in(written).unwrap_err();

        assert_eq!(problems.len(), 5);
        assert!(problems[0].contains("cannot be a picture's name"), "{}", problems[0]);
        assert!(problems[1].starts_with("Picture `a`: it needs a \"page\""), "{}", problems[1]);
        assert!(problems[2].contains("run off the page"), "{}", problems[2]);
        assert!(problems[3].contains("needs \"frames\""), "{}", problems[3]);
        assert!(problems[4].contains("\"motion\" is"), "{}", problems[4]);
    }

    #[test]
    fn a_file_that_is_not_a_list_of_pictures_is_refused() {
        assert!(pictures_written_in("not json").is_err());
        assert!(pictures_written_in("{}").is_err());
    }

    #[test]
    fn a_picture_over_empty_slots_on_a_real_page_is_fine() {
        assert_eq!(problems_with(&[looping_picture("reels", BOTTOM_LEFT_SLOT)], &layout_with_one_control()), Vec::<String>::new());
    }

    #[test]
    fn a_picture_may_not_cover_a_control_or_sit_on_a_page_that_does_not_exist() {
        let over_the_control = looping_picture("reels", SlotBlock { top_row: 0, left_column: 0, rows: 2, columns: 2 });
        let on_a_missing_page = Picture { page: "TAPE".to_string(), ..looping_picture("lamp", BOTTOM_LEFT_SLOT) };

        let problems = problems_with(&[over_the_control, on_a_missing_page], &layout_with_one_control());

        assert!(problems[0].contains("covers the control in row 1, column 1"), "{}", problems[0]);
        assert!(problems[1].contains("no page called `TAPE`. Its pages are: MAIN."), "{}", problems[1]);
    }

    #[test]
    fn a_backdrop_may_sit_behind_controls_but_not_with_too_many_frames() {
        let still_backdrop = Picture { placement: Placement::Backdrop, motion: Motion::Still, frames: 1, ..looping_picture("paper", BOTTOM_LEFT_SLOT) };
        let long_backdrop = Picture { placement: Placement::Backdrop, frames: 12, ..looping_picture("storm", BOTTOM_LEFT_SLOT) };

        let problems = problems_with(&[still_backdrop, long_backdrop], &layout_with_one_control());

        assert_eq!(problems, ["Picture `storm`: at this size it can have at most 7 frames."]);
    }

    #[test]
    fn two_pictures_cannot_share_a_name() {
        let pictures = [looping_picture("reels", BOTTOM_LEFT_SLOT), looping_picture("reels", SlotBlock { left_column: 1, ..BOTTOM_LEFT_SLOT })];

        let problems = problems_with(&pictures, &layout_with_one_control());

        assert_eq!(problems, ["Two pictures are called `reels`. Give each its own name."]);
    }
}
