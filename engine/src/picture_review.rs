//! Shows the AI what it made: each page of the screen as the MPC lays it out, and a contact sheet of
//! every picture it drew.
//!
//! Something the AI has already been shown is not shown again until it changes, because every image
//! shown is paid for again on each later step of the conversation.

use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use crate::agent::Illustration;
use crate::artwork::{drawn_file_of, drawn_pictures};
use crate::contact_sheet::{ContactSheet, contact_sheet_of};
use crate::page_view::{PageView, page_views};
use crate::pictures::Picture;

/// Said once above the pictures, so the AI knows what it is looking at and what to do about it.
pub const HOW_TO_READ_THE_PICTURES: &str = "Your pictures as drawn follow, each on a dark stand-in for the page. \
    Look at every one. If a shape is cut off, off-centre, too faint, too fussy, or a loop would jump where it joins, \
    redraw it and check again. A picture you have already seen is only shown again when it changes.";

/// Said once above the pages, so the AI judges each one as a whole.
pub const HOW_TO_READ_THE_PAGES: &str = "Each page of the screen follows, laid out as the MPC lays it out, with every     control at its starting setting and the lettering in a stand-in font. Judge each page as the musician will see it.     Is anything floating alone? Is part of the page empty by accident? Does each picture sit beside the control it     belongs to? Is the lettering readable over what is behind it? Fix what is wrong and check again. A page you have     already seen is only shown again when it changes.";

/// A page cannot share a name with a picture, because a picture's name has no space in it.
const PAGE_KEY_START: &str = "page ";

/// One of the plugin's pictures with the image file drawn for it.
struct DrawnPicture {
    picture: Picture,
    film_strip: Vec<u8>,
}

impl DrawnPicture {
    fn fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.picture.frames, &self.film_strip).hash(&mut hasher);
        hasher.finish()
    }
}

fn fingerprint_of(view: &PageView) -> u64 {
    let mut hasher = DefaultHasher::new();
    view.png.hash(&mut hasher);
    hasher.finish()
}

fn key_of(view: &PageView) -> String {
    format!("{PAGE_KEY_START}{}", view.page_name)
}

fn illustration_of_page(view: PageView) -> Illustration {
    Illustration { caption: format!("Page `{}` as the MPC lays it out. The real page is 1280 by 628; this is shown smaller.", view.page_name), png: view.png }
}

/// Remembers which drawings and pages have been shown, by name.
#[derive(Default)]
pub struct PictureReview {
    fingerprints_shown: HashMap<String, u64>,
}

impl PictureReview {
    /// The drawn pictures that have not been shown as they are now. They count as shown from here on.
    pub fn show_what_changed(&mut self, plugin: &Path) -> Vec<Illustration> {
        let changed: Vec<DrawnPicture> = drawn_pictures_of(plugin).into_iter().filter(|drawn| !self.has_shown(drawn)).collect();
        self.remember(&changed);
        changed.iter().filter_map(illustration_of).collect()
    }

    /// The pages of the screen that have not been shown as they look now. They count as shown from here on.
    pub fn show_changed_pages(&mut self, plugin: &Path) -> Vec<Illustration> {
        let is_new = |view: &PageView| self.fingerprints_shown.get(&key_of(view)) != Some(&fingerprint_of(view));
        let changed: Vec<PageView> = page_views(plugin).into_iter().filter(is_new).collect();
        self.fingerprints_shown.extend(changed.iter().map(|view| (key_of(view), fingerprint_of(view))));
        changed.into_iter().map(illustration_of_page).collect()
    }

    /// The pages as they look now, all of them or only the one named, whether or not they were shown before.
    /// They count as shown from here on.
    pub fn show_pages(&mut self, plugin: &Path, only: Option<&str>) -> Vec<Illustration> {
        let is_asked_for = |view: &PageView| only.is_none_or(|page_name| view.page_name == page_name);
        let asked_for: Vec<PageView> = page_views(plugin).into_iter().filter(is_asked_for).collect();
        self.fingerprints_shown.extend(asked_for.iter().map(|view| (key_of(view), fingerprint_of(view))));
        asked_for.into_iter().map(illustration_of_page).collect()
    }

    fn has_shown(&self, drawn: &DrawnPicture) -> bool {
        self.fingerprints_shown.get(&drawn.picture.name) == Some(&drawn.fingerprint())
    }

    fn remember(&mut self, shown: &[DrawnPicture]) {
        let fingerprints = shown.iter().map(|drawn| (drawn.picture.name.clone(), drawn.fingerprint()));
        self.fingerprints_shown.extend(fingerprints);
    }
}

fn drawn_pictures_of(plugin: &Path) -> Vec<DrawnPicture> {
    let with_its_image = |picture: Picture| {
        let film_strip = fs::read(drawn_file_of(plugin, &picture.name)).ok()?;
        Some(DrawnPicture { picture, film_strip })
    };
    drawn_pictures(plugin).into_iter().filter_map(with_its_image).collect()
}

fn illustration_of(drawn: &DrawnPicture) -> Option<Illustration> {
    let sheet = contact_sheet_of(&drawn.film_strip, drawn.picture.frames).ok()?;
    Some(Illustration { caption: caption_of(&drawn.picture, &sheet), png: sheet.png })
}

fn caption_of(picture: &Picture, sheet: &ContactSheet) -> String {
    let size = picture.frame_size();
    let frames = frames_in_words(&sheet.frames_shown, picture.frames);
    format!("Picture `{}`: {frames}. On the MPC each frame is {} by {}.", picture.name, size.width, size.height)
}

fn frames_in_words(shown: &[u32], frames: u32) -> String {
    match shown {
        [_only] if frames == 1 => "its one frame".to_string(),
        _ => format!("frames {} of {frames}, from top to bottom", listed(shown)),
    }
}

/// Numbers the way they are said aloud: "1, 4, 8 and 12".
fn listed(numbers: &[u32]) -> String {
    let words: Vec<String> = numbers.iter().map(u32::to_string).collect();
    match words.split_last() {
        Some((last, earlier)) if !earlier.is_empty() => format!("{} and {last}", earlier.join(", ")),
        _ => words.concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artwork::draw_pictures;
    use crate::film_strip::test_support::size_of;
    use crate::pictures::{DRAWINGS_FOLDER, PICTURES_FILE};
    use crate::workspace::test_support::TemporaryFolder;

    const SPINNING_REEL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 278 107"><rect x="130" y="5" width="18" height="40" fill="white" transform="rotate({90 * t} 139 53)"/></svg>"#;
    const WIDER_REEL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 278 107"><rect x="120" y="5" width="38" height="40" fill="white" transform="rotate({90 * t} 139 53)"/></svg>"#;
    const PAPER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 628"><rect width="1280" height="628" fill="tan"/></svg>"#;
    const REEL_AND_PAPER: &str = r#"{ "pictures": [
        { "name": "reel", "page": "MAIN", "where": { "row": 4, "column": 1 }, "motion": "loops", "frames": 12 },
        { "name": "paper", "page": "MAIN", "where": "backdrop" }
    ] }"#;

    fn plugin_with_drawn_pictures() -> TemporaryFolder {
        let plugin = TemporaryFolder::create("picture-review");
        fs::create_dir_all(plugin.0.join(DRAWINGS_FOLDER)).unwrap();
        fs::write(plugin.0.join("ui.json"), r#"{ "pages": [{ "name": "MAIN", "controls": ["Time"] }] }"#).unwrap();
        fs::write(plugin.0.join(PICTURES_FILE), REEL_AND_PAPER).unwrap();
        fs::write(plugin.0.join("pictures/reel.svg"), SPINNING_REEL).unwrap();
        fs::write(plugin.0.join("pictures/paper.svg"), PAPER).unwrap();
        draw_pictures(&plugin.0).unwrap();
        plugin
    }

    #[test]
    fn every_drawn_picture_is_shown_with_words_that_say_which_frames_it_shows() {
        let plugin = plugin_with_drawn_pictures();

        let shown = PictureReview::default().show_what_changed(&plugin.0);

        assert_eq!(shown[0].caption, "Picture `reel`: frames 1, 4, 8 and 12 of 12, from top to bottom. On the MPC each frame is 278 by 107.");
        assert_eq!(shown[1].caption, "Picture `paper`: its one frame. On the MPC each frame is 1280 by 628.");
        assert_eq!(size_of(&shown[0].png).0, 278);
        assert_eq!(size_of(&shown[1].png).0, 768);
    }

    #[test]
    fn a_picture_is_only_shown_again_once_it_has_changed() {
        let plugin = plugin_with_drawn_pictures();
        let mut review = PictureReview::default();
        review.show_what_changed(&plugin.0);

        let shown_when_nothing_changed = review.show_what_changed(&plugin.0);
        fs::write(plugin.0.join("pictures/reel.svg"), WIDER_REEL).unwrap();
        draw_pictures(&plugin.0).unwrap();
        let shown_after_a_redraw = review.show_what_changed(&plugin.0);

        assert!(shown_when_nothing_changed.is_empty());
        assert_eq!(shown_after_a_redraw.len(), 1);
        assert!(shown_after_a_redraw[0].caption.starts_with("Picture `reel`"));
    }

    #[test]
    fn a_plugin_without_pictures_shows_nothing() {
        let plugin = TemporaryFolder::create("picture-review-none");

        assert!(PictureReview::default().show_what_changed(&plugin.0).is_empty());
    }

    #[test]
    fn frame_numbers_are_listed_the_way_they_are_said() {
        assert_eq!(listed(&[1]), "1");
        assert_eq!(listed(&[1, 2]), "1 and 2");
        assert_eq!(listed(&[1, 4, 8, 12]), "1, 4, 8 and 12");
    }
}
