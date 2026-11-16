//! Draws a plugin's pictures into the image files the MPC plays, and finishes the screen the build made:
//! what the musician arranged by hand is applied to it, the artwork they laid behind each page is drawn
//! and put there, and the pictures are added.
//!
//! The drawn pictures go in the plugin's `assets/pictures` folder, which Akai's build copies next to the
//! screen file. The folder is emptied before every drawing, so it only ever holds the current pictures.
//! Everything that comes from the musician's own look is drawn straight beside the built screen file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::Pixmap;
use serde_json::Value;

use crate::arranged_screen::{self, ArrangedPage, arranged};
use crate::built_screen::{
    IMAGES_FOLDER, LOOK_FILE_START, PlacedPicture, Playback, SizedArt, page_art_file_name, sized_art_in, with_arrangement, with_headings_shown, with_page_art, with_pictures,
};
use crate::film_strip::{FilmStrip, FrameSize};
use crate::library;
use crate::look::{FULLY_SOLID, ItemKind, Look, look_of};
use crate::page_art;
use crate::picture_clock::CLOCK_CONTROL;
use crate::pictures::{Motion, Picture, pictures_of};
use crate::screen_layout::ScreenLayout;
use crate::sdk::built_screen_file;

const FOLDER_THE_BUILD_COPIES: &str = "assets";

/// Draws every picture afresh, at the size the musician gave it, and returns the pictures drawn, or every
/// reason one could not be. An arrangement or artwork that cannot go on the screen is reported here too,
/// before anything is built.
pub fn draw_pictures(plugin: &Path) -> Result<Vec<Picture>, Vec<String>> {
    remove_drawn_pictures(plugin).map_err(|error| vec![format!("The old pictures could not be cleared: {error}")])?;
    let pictures = pictures_of(plugin)?;
    let arrangement = arrangement_of(plugin, &pictures)?;
    let failures: Vec<String> = pictures.iter().filter_map(|picture| draw(plugin, picture, size_arranged_for(picture, &arrangement.pages)).err()).collect();
    if failures.is_empty() { Ok(pictures) } else { Err(failures) }
}

/// The picture of all the artwork the musician laid behind one page, as it will be shown on the MPC.
/// A page without artwork has none.
pub fn page_art_image(plugin: &Path, page_name: &str) -> Option<Vec<u8>> {
    let look = look_of(plugin).ok()?;
    let layers = look.artwork_of(page_name);
    if page_art::shows_anything(layers) { page_art::flattened(plugin, layers).ok() } else { None }
}

/// The pictures that have been drawn and are still wanted: what the preview can show.
pub fn drawn_pictures(plugin: &Path) -> Vec<Picture> {
    let wanted = pictures_of(plugin).unwrap_or_default();
    wanted.into_iter().filter(|picture| drawn_file_of(plugin, &picture.name).is_file()).collect()
}

/// The drawn image of one of the plugin's pictures. A name that is not one of its pictures gets nothing.
pub fn drawn_image(plugin: &Path, picture_name: &str) -> Option<Vec<u8>> {
    let is_a_picture = drawn_pictures(plugin).iter().any(|picture| picture.name == picture_name);
    if is_a_picture { fs::read(drawn_file_of(plugin, picture_name)).ok() } else { None }
}

pub fn drawn_file_of(plugin: &Path, picture_name: &str) -> PathBuf {
    drawn_folder_of(plugin).join(format!("{picture_name}.png"))
}

/// Draws kit images at other sizes beside the built screen, or says why that could not be done.
pub type SizedArtDrawer<'a> = &'a dyn Fn(&[SizedArt]) -> Result<(), String>;

/// Applies the musician's arrangement to the screen file the build made, puts their artwork behind each
/// page, and adds the plugin's pictures. `draw_sized_art` draws the knobs and faders they resized.
/// Returns whether the file was changed, which is when it needs checking again.
pub fn finish_built_screen(plugin: &Path, draw_sized_art: SizedArtDrawer) -> Result<bool, Vec<String>> {
    let pictures = pictures_of(plugin)?;
    let arrangement = arrangement_of(plugin, &pictures)?;
    let pages_with_art = arrangement.pages_with_art();
    let nothing_to_do = pictures.is_empty() && pages_with_art.is_empty() && !arrangement.pages.iter().any(needs_finishing);
    if nothing_to_do {
        return Ok(false);
    }
    let showing: Vec<Picture> = pictures.into_iter().filter(|picture| !arrangement.hides(picture)).collect();
    let placed = placed_on(&showing, &arrangement.pages, &built_control_names(plugin))?;
    let finished = FinishedScreen { arrangement: &arrangement, pages_with_art: &pages_with_art, placed: &placed };
    finished.write_beside_the_build(plugin, draw_sized_art).map_err(|problem| vec![problem])?;
    Ok(true)
}

/// A page needs finishing when the musician changed it, or when it has a heading, which the build leaves
/// in a form the MPC does not draw.
fn needs_finishing(page: &ArrangedPage) -> bool {
    page.is_changed() || page.items.iter().any(|item| item.kind == ItemKind::Heading)
}

/// The musician's look together with where it puts everything on the plugin's screen.
struct Arrangement {
    look: Look,
    pages: Vec<ArrangedPage>,
}

impl Arrangement {
    /// Whether the musician took this picture off its page, which keeps it off the MPC's screen as well.
    fn hides(&self, picture: &Picture) -> bool {
        self.pages.iter().any(|page| page.name == picture.page && page.hides(&picture.name))
    }

    /// The pages that have artwork to show behind their controls, counted from zero.
    fn pages_with_art(&self) -> Vec<usize> {
        let shows_art = |page: &ArrangedPage| page_art::shows_anything(self.look.artwork_of(&page.name));
        self.pages.iter().enumerate().filter(|(_, page)| shows_art(page)).map(|(position, _)| position).collect()
    }
}

/// Where everything on the plugin's screen goes, or every reason the look cannot be used.
fn arrangement_of(plugin: &Path, pictures: &[Picture]) -> Result<Arrangement, Vec<String>> {
    let look = look_of(plugin).map_err(|problem| vec![problem])?;
    let pages = arranged(&ScreenLayout::of_plugin(plugin), pictures, &look);
    let mut problems = arranged_screen::problems_with(&pages);
    problems.extend(pages.iter().flat_map(|page| page_art::problems_with(plugin, &page.name, look.artwork_of(&page.name))));
    if problems.is_empty() { Ok(Arrangement { look, pages }) } else { Err(problems) }
}

/// The size the picture is drawn at: its own, unless the musician resized it.
fn size_arranged_for(picture: &Picture, pages: &[ArrangedPage]) -> FrameSize {
    let on_its_page = pages.iter().filter(|page| page.name == picture.page).flat_map(|page| &page.pictures);
    let arranged = on_its_page.into_iter().find(|arranged| arranged.name == picture.name);
    arranged.map_or_else(|| picture.frame_size(), |arranged| FrameSize { width: arranged.area.width, height: arranged.area.height })
}

fn draw(plugin: &Path, picture: &Picture, frame_size: FrameSize) -> Result<(), String> {
    let about = |problem: String| format!("Picture `{}`: {problem}", picture.name);
    let drawing_path = picture.drawing_path();
    let drawing = fs::read_to_string(plugin.join(&drawing_path)).map_err(|_| about(format!("its drawing, {drawing_path}, has not been written.")))?;
    let strip = FilmStrip { drawing: &drawing, frame_size, frames: picture.frames, timing: picture.timing() };
    let png = strip.as_png().map_err(about)?;
    keep_drawn(plugin, picture, &png).map_err(|error| about(format!("it could not be saved: {error}")))
}

fn keep_drawn(plugin: &Path, picture: &Picture, png: &[u8]) -> io::Result<()> {
    fs::create_dir_all(drawn_folder_of(plugin))?;
    fs::write(drawn_file_of(plugin, &picture.name), png)
}

fn remove_drawn_pictures(plugin: &Path) -> io::Result<()> {
    match fs::remove_dir_all(drawn_folder_of(plugin)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn drawn_folder_of(plugin: &Path) -> PathBuf {
    plugin.join(FOLDER_THE_BUILD_COPIES).join(IMAGES_FOLDER)
}

fn placed_on<'a>(pictures: &'a [Picture], pages: &[ArrangedPage], control_names: &[String]) -> Result<Vec<PlacedPicture<'a>>, Vec<String>> {
    let (placed, problems): (Vec<_>, Vec<_>) = pictures.iter().map(|picture| placed(picture, pages, control_names)).partition(Result::is_ok);
    if problems.is_empty() { Ok(placed.into_iter().flatten().collect()) } else { Err(problems.into_iter().filter_map(Result::err).collect()) }
}

fn placed<'a>(picture: &'a Picture, pages: &[ArrangedPage], control_names: &[String]) -> Result<PlacedPicture<'a>, String> {
    let page_position = pages.iter().position(|page| page.name == picture.page);
    let page_position = page_position.ok_or_else(|| format!("Picture `{}`: ui.json has no page called `{}`.", picture.name, picture.page))?;
    let arranged = pages[page_position].pictures.iter().find(|arranged| arranged.name == picture.name);
    let arranged = arranged.ok_or_else(|| format!("Picture `{}` is not on page `{}`.", picture.name, picture.page))?;
    Ok(PlacedPicture { picture, page_position, playback: playback_of(picture, control_names)?, area: arranged.area, depth: arranged.depth })
}

fn playback_of(picture: &Picture, control_names: &[String]) -> Result<Playback, String> {
    match &picture.motion {
        Motion::Still => Ok(Playback::Still),
        Motion::Loops => number_of(CLOCK_CONTROL, control_names).map(Playback::Following).ok_or_else(|| needs_the_clock(picture)),
        Motion::Follows(control) => number_of(control, control_names).map(Playback::Following).ok_or_else(|| {
            format!("Picture `{}`: it follows `{control}`, but the plugin's controls are: {}.", picture.name, control_names.join(", "))
        }),
    }
}

fn needs_the_clock(picture: &Picture) -> String {
    format!(
        "Picture `{}` plays on its own, and an MPC only moves a picture when one of the plugin's controls changes. \
         Give the plugin the hidden `{CLOCK_CONTROL}` control and turn it from `process()`, the way `mp-forge/pictures.md` in the reference library shows \
         under \"Pictures that play on their own\".",
        picture.name
    )
}

/// The number the built plugin gives a control, for whoever has to watch it.
pub fn built_control_number(plugin: &Path, control: &str) -> Option<u32> {
    number_of(control, &built_control_names(plugin))
}

/// A control's number is its place in the list. The exact name wins; failing that, the same name in other capitals.
fn number_of(control: &str, control_names: &[String]) -> Option<u32> {
    let exact = control_names.iter().position(|name| name == control);
    let position = exact.or_else(|| control_names.iter().position(|name| name.eq_ignore_ascii_case(control)))?;
    u32::try_from(position).ok()
}

/// The names of the plugin's controls, in the order the built plugin reports them.
fn built_control_names(plugin: &Path) -> Vec<String> {
    let reported = library::built_parameters(plugin).unwrap_or(Value::Null);
    let controls = reported["params"].as_array().map(Vec::as_slice).unwrap_or_default();
    controls.iter().map(|control| control["name"].as_str().unwrap_or_default().to_string()).collect()
}

/// Everything that turns the screen the build made into the finished one.
struct FinishedScreen<'a> {
    arrangement: &'a Arrangement,
    pages_with_art: &'a [usize],
    placed: &'a [PlacedPicture<'a>],
}

impl FinishedScreen<'_> {
    /// The arrangement goes first: it finds what the generator wrote by where it sits, before anything else is in the way.
    /// The artwork goes in before the pictures, so that a backdrop picture ends up behind it.
    fn written_over(&self, as_built: &str) -> Result<String, String> {
        let arranged = with_headings_shown(&with_arrangement(as_built, &self.arrangement.pages)?)?;
        with_pictures(&with_page_art(&arranged, self.pages_with_art)?, self.placed)
    }

    fn write_beside_the_build(&self, plugin: &Path, draw_sized_art: SizedArtDrawer) -> Result<(), String> {
        let screen_file = built_screen_file(plugin).ok_or("The build made no screen to finish.")?;
        let beside_it = screen_file.parent().ok_or("The build made no screen to finish.")?;
        let as_built = fs::read_to_string(&screen_file).map_err(|error| format!("The built screen file could not be read: {error}"))?;
        let finished = self.written_over(&as_built)?;
        remove_what_an_earlier_look_drew(beside_it).map_err(|error| format!("The old artwork could not be cleared: {error}"))?;
        self.draw_page_art(plugin, beside_it)?;
        self.fade_pictures(plugin, beside_it)?;
        let sized_art = sized_art_in(&finished);
        if !sized_art.is_empty() {
            draw_sized_art(&sized_art)?;
        }
        fs::write(&screen_file, finished).map_err(|error| format!("The built screen file could not be saved: {error}"))
    }

    /// A picture the musician made see-through is faded in the copy the build put beside the screen.
    /// The picture the plugin keeps stays solid, so it can be made solid again.
    fn fade_pictures(&self, plugin: &Path, beside_the_screen: &Path) -> Result<(), String> {
        let see_through = self.arrangement.pages.iter().flat_map(|page| &page.pictures).filter(|picture| picture.opacity < FULLY_SOLID);
        for picture in see_through {
            let solid = fs::read(drawn_file_of(plugin, &picture.name)).map_err(|error| format!("Picture `{}` could not be read: {error}", picture.name))?;
            let built_copy = beside_the_screen.join(IMAGES_FOLDER).join(format!("{}.png", picture.name));
            fs::write(built_copy, faded(&solid, picture.opacity)?).map_err(|error| format!("Picture `{}` could not be saved: {error}", picture.name))?;
        }
        Ok(())
    }

    fn draw_page_art(&self, plugin: &Path, beside_the_screen: &Path) -> Result<(), String> {
        for page_position in self.pages_with_art {
            let layers = self.arrangement.look.artwork_of(&self.arrangement.pages[*page_position].name);
            let picture = page_art::flattened(plugin, layers)?;
            fs::write(beside_the_screen.join(page_art_file_name(*page_position)), picture).map_err(|error| format!("The page's artwork could not be saved: {error}"))?;
        }
        Ok(())
    }
}

/// A picture with every pixel keeping only this share of itself, out of 100.
pub fn faded(png: &[u8], opacity: u32) -> Result<Vec<u8>, String> {
    let mut pixels = Pixmap::decode_png(png).map_err(|error| format!("A picture could not be read: {error}"))?;
    for channel in pixels.data_mut() {
        *channel = ((u32::from(*channel) * opacity + FULLY_SOLID / 2) / FULLY_SOLID) as u8;
    }
    pixels.encode_png().map_err(|error| format!("A picture could not be saved: {error}"))
}

fn remove_what_an_earlier_look_drew(beside_the_screen: &Path) -> io::Result<()> {
    for entry in fs::read_dir(beside_the_screen)? {
        let path = entry?.path();
        let from_a_look = path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(LOOK_FILE_START));
        if from_a_look && path.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::film_strip::test_support::size_of;
    use crate::look::LOOK_FILE;
    use crate::ordered_json::Json;
    use crate::pictures::{DRAWINGS_FOLDER, PICTURES_FILE};
    use crate::workspace::test_support::TemporaryFolder;

    const ONE_PAGE: &str = r#"{ "pages": [{ "name": "MAIN", "controls": ["Time"] }] }"#;
    const SPINNING_REEL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="45" y="5" width="10" height="40" fill="white" transform="rotate({360 * t} 50 50)"/></svg>"#;
    const REEL_IN_THE_CORNER: &str = r#"{ "name": "reel", "page": "MAIN", "where": { "row": 4, "column": 1 }, "motion": "loops", "frames": 3 }"#;
    const BUILT_SCREEN: &str = r#"{"pageData": {"version": 1, "tabs": [{"version": 1, "tabName": "MAIN", "componentDefinition": {"version": 2, "componentsData": []}}]}}"#;

    fn no_sized_art(asked_for: &[SizedArt]) -> Result<(), String> {
        Err(format!("No resized art was expected, but {} were asked for.", asked_for.len()))
    }

    fn plugin_with_pictures(pictures: &str) -> TemporaryFolder {
        let plugin = TemporaryFolder::create("artwork");
        fs::create_dir_all(plugin.0.join(DRAWINGS_FOLDER)).unwrap();
        fs::write(plugin.0.join("ui.json"), ONE_PAGE).unwrap();
        fs::write(plugin.0.join(PICTURES_FILE), format!(r#"{{ "pictures": [{pictures}] }}"#)).unwrap();
        fs::write(plugin.0.join("pictures/reel.svg"), SPINNING_REEL).unwrap();
        plugin
    }

    fn add_built_screen(plugin: &TemporaryFolder, control_names: &[&str]) -> PathBuf {
        let skins = plugin.0.join("build/Tests - VST - Reels/Plugin Skins");
        fs::create_dir_all(&skins).unwrap();
        fs::write(skins.join("TUI.json"), BUILT_SCREEN).unwrap();
        let controls: Vec<Value> = control_names.iter().map(|name| serde_json::json!({ "name": name })).collect();
        fs::write(plugin.0.join("build/params.json"), serde_json::json!({ "params": controls }).to_string()).unwrap();
        skins.join("TUI.json")
    }

    #[test]
    fn each_picture_is_drawn_as_a_strip_of_its_frames_at_the_size_of_its_slots() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);

        let drawn = draw_pictures(&plugin.0).unwrap();

        let png = fs::read(drawn_file_of(&plugin.0, "reel")).unwrap();
        assert_eq!(drawn.len(), 1);
        assert_eq!(size_of(&png), (278, 107 * 3));
        assert_eq!(drawn_pictures(&plugin.0), drawn);
        assert_eq!(drawn_image(&plugin.0, "reel"), Some(png));
    }

    #[test]
    fn only_the_plugins_own_pictures_can_be_fetched() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        draw_pictures(&plugin.0).unwrap();
        fs::write(plugin.0.join("assets/secret.png"), "not a picture").unwrap();

        assert!(drawn_image(&plugin.0, "../secret").is_none());
        assert!(drawn_image(&plugin.0, "lamp").is_none());
    }

    #[test]
    fn pictures_that_are_no_longer_wanted_are_cleared_away() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        draw_pictures(&plugin.0).unwrap();
        fs::remove_file(plugin.0.join(PICTURES_FILE)).unwrap();

        let drawn = draw_pictures(&plugin.0).unwrap();

        assert!(drawn.is_empty());
        assert!(!drawn_file_of(&plugin.0, "reel").exists());
        assert!(drawn_pictures(&plugin.0).is_empty());
    }

    #[test]
    fn a_picture_without_a_drawing_or_with_a_broken_one_says_so() {
        let lamp = r#"{ "name": "lamp", "page": "MAIN", "where": { "row": 4, "column": 2 } }"#;
        let plugin = plugin_with_pictures(&format!("{REEL_IN_THE_CORNER}, {lamp}"));
        fs::write(plugin.0.join("pictures/reel.svg"), "<svg><rect x=\"{spin}\"/></svg>").unwrap();

        let problems = draw_pictures(&plugin.0).unwrap_err();

        assert!(problems[0].starts_with("Picture `reel`: Frame 1 could not be drawn"), "{}", problems[0]);
        assert_eq!(problems[1], "Picture `lamp`: its drawing, pictures/lamp.svg, has not been written.");
    }

    #[test]
    fn drawn_pictures_are_added_to_the_screen_the_build_made() {
        let follower = r#"{ "name": "reel", "page": "MAIN", "where": { "row": 4, "column": 1 }, "motion": { "follows": "drive" }, "frames": 3 }"#;
        let plugin = plugin_with_pictures(follower);
        let screen_file = add_built_screen(&plugin, &["Volume", "Drive"]);

        let changed = finish_built_screen(&plugin.0, &no_sized_art).unwrap();

        let screen = Json::parse(&fs::read_to_string(screen_file).unwrap()).unwrap().written();
        assert!(changed);
        assert!(screen.contains("\"image\": \"pictures/reel.png\""), "{screen}");
        assert!(screen.contains("\"handleName\": \"Parameter 1\""), "{screen}");
    }

    #[test]
    fn a_picture_that_plays_on_its_own_follows_the_plugins_clock() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let screen_file = add_built_screen(&plugin, &["Volume", "Drive", "picture clock"]);

        finish_built_screen(&plugin.0, &no_sized_art).unwrap();

        let screen = Json::parse(&fs::read_to_string(screen_file).unwrap()).unwrap().written();
        assert!(screen.contains("\"handleName\": \"Parameter 2\""), "{screen}");
        assert!(screen.contains("\"animated\": false"), "{screen}");
    }

    #[test]
    fn a_picture_that_plays_on_its_own_in_a_plugin_without_a_clock_is_refused() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        add_built_screen(&plugin, &["Volume", "Drive"]);

        let problems = finish_built_screen(&plugin.0, &no_sized_art).unwrap_err();

        assert!(problems[0].starts_with("Picture `reel` plays on its own, and an MPC only moves a picture when"), "{problems:?}");
        assert!(problems[0].contains("`Picture Clock`"), "{problems:?}");
    }

    #[test]
    fn a_picture_that_follows_a_control_the_plugin_does_not_have_is_refused() {
        let follower = r#"{ "name": "reel", "page": "MAIN", "where": { "row": 4, "column": 1 }, "motion": { "follows": "Wobble" }, "frames": 3 }"#;
        let plugin = plugin_with_pictures(follower);
        add_built_screen(&plugin, &["Volume", "Drive"]);

        let problems = finish_built_screen(&plugin.0, &no_sized_art).unwrap_err();

        assert_eq!(problems, ["Picture `reel`: it follows `Wobble`, but the plugin's controls are: Volume, Drive."]);
    }

    #[test]
    fn a_plugin_without_pictures_leaves_the_built_screen_alone() {
        let plugin = TemporaryFolder::create("artwork-none");
        let screen_file = add_built_screen(&plugin, &["Volume"]);

        assert!(!finish_built_screen(&plugin.0, &no_sized_art).unwrap());
        assert_eq!(fs::read_to_string(screen_file).unwrap(), BUILT_SCREEN);
    }

    const TIME_MOVED_AND_RENAMED: &str = r#"{ "pages": [{ "page": "MAIN", "items": [
        { "kind": "control", "name": "Time", "place": { "x": 600, "y": 300 }, "shownName": "SPEED" },
        { "kind": "picture", "name": "reel", "place": { "x": 900, "y": 500 } }
    ]}]}"#;
    const GENERATED_TIME_KNOB: &str = r#"{"pageData": {"version": 1, "tabs": [{"version": 1, "tabName": "MAIN",
        "componentDefinition": {"version": 2, "componentsData": [
            {"componentData": {"name": "Time", "type": "knobUni", "data": {"handleName": "Data"}},
             "handle remapping": {"version": 1, "map": [{"key": "Data", "value": "Parameter 1"}]}, "bounds": {"bounds": "23 45 278 107"}}
        ]},
        "qlinkBoundsData": ["38 0 248 624", "358 0 248 624", "678 0 248 624", "998 0 248 624"]}]}}"#;

    #[test]
    fn what_the_musician_arranged_is_applied_to_the_screen_the_build_made() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let screen_file = add_built_screen(&plugin, &["Volume", "Time", "Picture Clock"]);
        fs::write(&screen_file, GENERATED_TIME_KNOB).unwrap();
        fs::write(plugin.0.join(LOOK_FILE), TIME_MOVED_AND_RENAMED).unwrap();

        let changed = finish_built_screen(&plugin.0, &no_sized_art).unwrap();

        let screen = fs::read_to_string(screen_file).unwrap();
        assert!(changed);
        assert!(screen.contains("\"bounds\": \"600 300 278 107\""), "{screen}");
        assert!(screen.contains("\"value\": \"WithName[SPEED]/Parameter 1\""), "{screen}");
        assert!(screen.contains("\"bounds\": \"900 500 278 107\""), "{screen}");
        assert!(screen.contains("\"594 294 290 119\""), "{screen}");
    }

    const RED_BEHIND_MAIN: &str = r##"{ "pages": [{ "page": "MAIN", "artwork": [
        { "kind": "block", "id": 1, "area": { "x": 0, "y": 0, "width": 1280, "height": 628 }, "colour": "#ff0000" }
    ]}]}"##;

    #[test]
    fn artwork_behind_a_page_is_drawn_beside_the_built_screen_and_shown_behind_everything() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let screen_file = add_built_screen(&plugin, &["Volume", "Time", "Picture Clock"]);
        fs::write(&screen_file, GENERATED_TIME_KNOB).unwrap();
        fs::write(plugin.0.join(LOOK_FILE), RED_BEHIND_MAIN).unwrap();
        fs::write(screen_file.with_file_name("look-page-7.png"), "left by an earlier look").unwrap();

        let changed = finish_built_screen(&plugin.0, &no_sized_art).unwrap();

        let screen = Json::parse(&fs::read_to_string(&screen_file).unwrap()).unwrap().written();
        let drawn = fs::read(screen_file.with_file_name("look-page-1.png")).unwrap();
        assert!(changed && !screen_file.with_file_name("look-page-7.png").exists());
        assert_eq!((size_of(&drawn), crate::film_strip::test_support::pixel_of(&drawn, 640, 300)), ((1280, 628), [255, 0, 0, 255]));
        assert!(screen.find("look-page-1.png").unwrap() < screen.find("\"Time\"").unwrap(), "{screen}");
        assert_eq!(page_art_image(&plugin.0, "MAIN"), Some(drawn));
        assert_eq!(page_art_image(&plugin.0, "MORE"), None);
    }

    #[test]
    fn a_knob_the_musician_resized_has_its_image_drawn_at_the_new_size() {
        let plugin = TemporaryFolder::create("artwork-sized");
        fs::write(plugin.0.join("ui.json"), ONE_PAGE).unwrap();
        let screen_file = add_built_screen(&plugin, &["Volume", "Time"]);
        let with_knob_defined = GENERATED_TIME_KNOB.replace(r#"{"pageData": {"version": 1, "tabs""#, r#"{"pageData": {"version": 1, "componentDefinitions": {"localComponentDefinitions": [
            {"key": "knobUni", "value": {"componentsData": [
                {"componentData": {"name": "knob", "type": "Knob", "data": {"knobType": "FilmStrip", "filmStrip": "knob_uni.png", "numFrames": 127}}, "bounds": {"bounds": "8 8 90 90"}}
            ]}}]}, "tabs""#);
        fs::write(&screen_file, with_knob_defined).unwrap();
        let bigger_knob = r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Time", "parts": { "art": { "x": 0, "y": 0, "width": 150, "height": 150 } } }] }] }"#;
        fs::write(plugin.0.join(LOOK_FILE), bigger_knob).unwrap();
        let asked_for = std::cell::RefCell::new(Vec::new());

        let changed = finish_built_screen(&plugin.0, &|sized| {
            asked_for.borrow_mut().extend(sized.iter().map(SizedArt::file_name));
            Ok(())
        });

        assert_eq!(changed, Ok(true));
        assert_eq!(*asked_for.borrow(), ["look-knob_uni-150x150-107.png"]);
        assert!(fs::read_to_string(screen_file).unwrap().contains("\"filmStrip\": \"look-knob_uni-150x150-107.png\""));
    }

    #[test]
    fn a_picture_the_musician_resized_is_drawn_at_the_new_size() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let resized = r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "picture", "name": "reel", "size": { "width": 400, "height": 200 }, "place": { "x": 100, "y": 100 } }] }] }"#;
        fs::write(plugin.0.join(LOOK_FILE), resized).unwrap();

        draw_pictures(&plugin.0).unwrap();

        assert_eq!(size_of(&fs::read(drawn_file_of(&plugin.0, "reel")).unwrap()), (400, 200 * 3));
    }

    #[test]
    fn artwork_that_cannot_be_drawn_stops_the_pictures_being_drawn() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let gone = r#"{ "pages": [{ "page": "MAIN", "artwork": [{ "kind": "picture", "id": 3, "file": "art-3.png", "area": { "x": 0, "y": 0, "width": 10, "height": 10 } }] }] }"#;
        fs::write(plugin.0.join(LOOK_FILE), gone).unwrap();

        let problems = draw_pictures(&plugin.0).unwrap_err();

        assert!(problems[0].contains("a picture (art-3.png) is no longer in the plugin"), "{problems:?}");
    }

    #[test]
    fn a_heading_alone_is_enough_to_change_the_built_screen_because_the_mpc_would_not_draw_it() {
        let plugin = TemporaryFolder::create("artwork-heading");
        fs::write(plugin.0.join("ui.json"), r#"{ "pages": [{ "name": "MAIN", "controls": ["Time", { "widget": "text", "text": "TAPE" }] }] }"#).unwrap();
        let screen_file = add_built_screen(&plugin, &["Volume", "Time"]);
        let generated_heading = r#"{"pageData": {"version": 1, "tabs": [{"version": 1, "tabName": "MAIN", "componentDefinition": {"version": 2, "componentsData": [
            {"componentData": {"name": "text 1", "type": "Label", "data": {"type": "Text", "handleName": "Data", "text": "TAPE"}},
             "handle remapping": {"version": 1, "map": []}, "bounds": {"bounds": "343 45 278 107"}}
        ]}}]}}"#;
        fs::write(&screen_file, generated_heading).unwrap();

        assert_eq!(finish_built_screen(&plugin.0, &no_sized_art), Ok(true));
        assert!(fs::read_to_string(screen_file).unwrap().contains("\"value\": \"Parameter 0\""));
    }

    #[test]
    fn an_arrangement_alone_is_enough_to_change_the_built_screen() {
        let plugin = TemporaryFolder::create("artwork-arranged");
        fs::write(plugin.0.join("ui.json"), ONE_PAGE).unwrap();
        let screen_file = add_built_screen(&plugin, &["Volume", "Time"]);
        fs::write(&screen_file, GENERATED_TIME_KNOB).unwrap();
        fs::write(plugin.0.join(LOOK_FILE), r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Time", "shownName": "SPEED" }] }] }"#).unwrap();

        assert!(finish_built_screen(&plugin.0, &no_sized_art).unwrap());
        assert!(fs::read_to_string(screen_file).unwrap().contains("WithName[SPEED]/Parameter 1"));
    }

    #[test]
    fn an_arrangement_that_cannot_go_on_the_screen_stops_the_pictures_being_drawn() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        let off_the_page = r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Time", "place": { "x": 1100, "y": 45 } }] }] }"#;
        fs::write(plugin.0.join(LOOK_FILE), off_the_page).unwrap();

        let problems = draw_pictures(&plugin.0).unwrap_err();

        assert_eq!(problems, ["On page `MAIN`, `Time` hangs off the edge of the screen. Move it back onto the page."]);
    }

    #[test]
    fn a_look_file_that_cannot_be_read_is_a_problem_not_a_silent_reset() {
        let plugin = plugin_with_pictures(REEL_IN_THE_CORNER);
        fs::write(plugin.0.join(LOOK_FILE), "not json").unwrap();

        assert!(draw_pictures(&plugin.0).unwrap_err()[0].contains("look.json is not valid JSON"));
    }
}
