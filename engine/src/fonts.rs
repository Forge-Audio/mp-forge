//! The fonts on the musician's computer, for lettering the workbench draws itself.
//!
//! Words drawn here end up as pixels in a picture, so the MPC never needs the font.

use std::sync::{Arc, OnceLock};

use resvg::usvg::{Options, fontdb};

use crate::layers::is_plain_font_name;

/// Options for reading a drawing whose lettering uses this computer's fonts.
pub fn options_with_this_computers_fonts() -> Options<'static> {
    Options { fontdb: Arc::clone(this_computers_fonts()), ..Options::default() }
}

/// The name of every font family on this computer, in alphabetical order.
pub fn font_names() -> Vec<String> {
    let mut names: Vec<String> = this_computers_fonts().faces().filter_map(family_name).filter(|name| is_for_people(name)).collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

pub fn has_font(name: &str) -> bool {
    this_computers_fonts().faces().filter_map(family_name).any(|known| known.eq_ignore_ascii_case(name))
}

fn family_name(face: &fontdb::FaceInfo) -> Option<String> {
    face.families.first().map(|(name, _)| name.clone())
}

/// Fonts whose names start with a full stop are the system's own and are kept out of font menus.
/// So is any font whose name could not be written into a drawing as it is.
fn is_for_people(name: &str) -> bool {
    !name.starts_with('.') && is_plain_font_name(name)
}

/// The fonts are found once, the first time any lettering is drawn.
fn this_computers_fonts() -> &'static Arc<fontdb::Database> {
    static FONTS_FOUND: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS_FOUND.get_or_init(|| {
        let mut fonts = fontdb::Database::new();
        fonts.load_system_fonts();
        Arc::new(fonts)
    })
}

#[cfg(test)]
pub mod test_support {
    use super::font_names;

    /// A font this computer certainly has, or nothing on a computer with no fonts at all.
    pub fn any_font() -> Option<String> {
        font_names().into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fonts_listed_are_ones_that_can_be_found_again_in_any_capitals() {
        let Some(name) = test_support::any_font() else { return };

        assert!(has_font(&name) && has_font(&name.to_uppercase()));
        assert!(!has_font("No Such Font On Any Computer 12345"));
    }

    #[test]
    fn the_list_is_in_order_without_repeats_or_hidden_system_fonts() {
        let names = font_names();

        let mut tidy = names.clone();
        tidy.sort_by_key(|name| name.to_lowercase());
        tidy.dedup();
        assert_eq!(names, tidy);
        assert!(names.iter().all(|name| !name.starts_with('.')));
    }
}
