//! Finishes the knob file the build made (`Q-Links.json`), which tells the MPC what each of its sixteen
//! hardware knobs turns on each page.
//!
//! Akai's generator gives every control the knob beside its slot. Where the musician chose other knobs,
//! that page is written again their way. A page they left alone stays exactly as the generator wrote it.

use serde_json::Number;

use crate::arranged_screen::{ArrangedItem, ArrangedPage};
use crate::look::ItemKind;
use crate::ordered_json::Json;

pub const KNOB_FILE: &str = "Q-Links.json";

const KNOB_NAME_START: &str = "Q-Link ";
const TURNS_NOTHING: i32 = -1;

/// Gives the number the built plugin knows a control by, or nothing when it has no such control.
pub type ControlNumbers<'a> = &'a dyn Fn(&str) -> Option<u32>;

/// Returns the knob file with every page where the musician chose a knob written the way they chose.
pub fn with_chosen_knobs(knob_file: &str, pages: &[ArrangedPage], number_of: ControlNumbers) -> Result<String, String> {
    let mut knobs = Json::parse(knob_file).map_err(|error| format!("The built knob file could not be read: {error}"))?;
    for (page_position, page) in pages.iter().enumerate().filter(|(_, page)| page.has_a_chosen_knob()) {
        let of_the_page = knobs_of_page(&mut knobs, page_position).ok_or_else(|| format!("The built knob file has no page {}.", page_position + 1))?;
        for (knob_name, turned) in of_the_page.iter_mut() {
            *turned = what_turns(knob_name, page, number_of)?;
        }
    }
    Ok(knobs.written())
}

/// The sixteen knobs of one page, each with the number of the control it turns.
fn knobs_of_page(knobs: &mut Json, page_position: usize) -> Option<&mut Vec<(String, Json)>> {
    let tab = Json::from(page_position as u32 + 1);
    let pages = knobs.field_mut("Screen Mode Q-Links")?.field_mut("map")?.items_mut()?;
    let page = pages.iter_mut().find(|page| page.field("Tab") == Some(&tab))?;
    match page.field_mut("Q-Links")? {
        Json::Object(of_the_page) => Some(of_the_page),
        _ => None,
    }
}

fn what_turns(knob_name: &str, page: &ArrangedPage, number_of: ControlNumbers) -> Result<Json, String> {
    let knob = knob_name.strip_prefix(KNOB_NAME_START).and_then(|number| number.parse::<u32>().ok());
    match page.items.iter().find(|item| item.kind == ItemKind::Control && item.knob.is_some() && item.knob == knob) {
        Some(control) => number_of(&control.name).map(Json::from).ok_or_else(|| not_in_the_built_plugin(control, page)),
        None => Ok(Json::Number(Number::from(TURNS_NOTHING))),
    }
}

fn not_in_the_built_plugin(control: &ArrangedItem, page: &ArrangedPage) -> String {
    format!("On page `{}`, `{}` was given a hardware knob, but the built plugin has no control of that name.", page.name, control.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arranged_screen::test_support::{control_in_slot, page_of};

    const GENERATED: &str = r#"{"version": 4, "info": {"version": 1, "type": "CompleteDescription"},
        "Screen Mode Q-Links": {"version": 4, "map": [
            {"Tab": 1, "SubTab": 1, "Bank Direction": "Column", "Q-Links": {
                "Q-Link 1": -1, "Q-Link 2": -1, "Q-Link 3": -1, "Q-Link 4": -1, "Q-Link 5": -1, "Q-Link 6": -1, "Q-Link 7": -1, "Q-Link 8": -1,
                "Q-Link 9": -1, "Q-Link 10": -1, "Q-Link 11": -1, "Q-Link 12": -1, "Q-Link 13": 1, "Q-Link 14": 2, "Q-Link 15": -1, "Q-Link 16": -1}},
            {"Tab": 2, "SubTab": 1, "Bank Direction": "Column", "Q-Links": {
                "Q-Link 1": -1, "Q-Link 2": -1, "Q-Link 3": -1, "Q-Link 4": -1, "Q-Link 5": -1, "Q-Link 6": -1, "Q-Link 7": -1, "Q-Link 8": -1,
                "Q-Link 9": -1, "Q-Link 10": -1, "Q-Link 11": -1, "Q-Link 12": -1, "Q-Link 13": 3, "Q-Link 14": -1, "Q-Link 15": -1, "Q-Link 16": -1}}]},
        "Program Mode Q-Links": {"Q-Link 1": 0, "Q-Link 2": 1}}"#;

    fn number_in_the_plugin(control: &str) -> Option<u32> {
        ["Volume", "Time", "Mix", "Tone"].iter().position(|name| *name == control).map(|position| position as u32)
    }

    fn turned_by(control: &str, slot: usize, knob: u32) -> ArrangedItem {
        ArrangedItem { knob: Some(knob), ..control_in_slot(control, slot) }
    }

    fn knob_of_page(knob_file: &str, page_position: usize, knob: u32) -> i64 {
        let written: serde_json::Value = serde_json::from_str(knob_file).unwrap();
        written["Screen Mode Q-Links"]["map"][page_position]["Q-Links"][format!("Q-Link {knob}")].as_i64().unwrap()
    }

    #[test]
    fn a_page_with_chosen_knobs_is_written_the_way_the_musician_chose() {
        let main = page_of("MAIN", vec![turned_by("Time", 0, 14), turned_by("Mix", 1, 5)]);

        let finished = with_chosen_knobs(GENERATED, &[main, page_of("MORE", vec![control_in_slot("Tone", 0)])], &number_in_the_plugin).unwrap();

        assert_eq!([13, 14, 5, 1].map(|knob| knob_of_page(&finished, 0, knob)), [-1, 1, 2, -1]);
        assert_eq!(knob_of_page(&finished, 1, 13), 3);
    }

    #[test]
    fn with_no_knob_chosen_the_file_says_what_the_generator_wrote() {
        let pages = [page_of("MAIN", vec![control_in_slot("Time", 0), control_in_slot("Mix", 1)])];

        let finished = with_chosen_knobs(GENERATED, &pages, &number_in_the_plugin).unwrap();

        assert_eq!(Json::parse(&finished), Json::parse(GENERATED));
    }

    #[test]
    fn the_knobs_stay_in_the_order_the_mpc_reads_them() {
        let main = page_of("MAIN", vec![turned_by("Time", 0, 14), turned_by("Mix", 1, 13)]);

        let finished = with_chosen_knobs(GENERATED, &[main], &number_in_the_plugin).unwrap();

        let first_page = finished.split("\"Tab\": 2").next().unwrap();
        let places: Vec<usize> = (1..=16).map(|knob| first_page.find(&format!("\"Q-Link {knob}\"")).unwrap()).collect();
        assert!(places.is_sorted());
    }

    #[test]
    fn a_chosen_knob_for_a_control_the_built_plugin_lacks_is_refused() {
        let main = page_of("MAIN", vec![turned_by("Wobble", 0, 14)]);

        let refusal = with_chosen_knobs(GENERATED, &[main], &number_in_the_plugin).unwrap_err();

        assert_eq!(refusal, "On page `MAIN`, `Wobble` was given a hardware knob, but the built plugin has no control of that name.");
    }

    #[test]
    fn a_knob_file_without_the_page_is_refused() {
        let pages = [page_of("MAIN", Vec::new()), page_of("MORE", Vec::new()), page_of("LAST", vec![turned_by("Time", 0, 14)])];

        assert_eq!(with_chosen_knobs(GENERATED, &pages, &number_in_the_plugin).unwrap_err(), "The built knob file has no page 3.");
        assert!(with_chosen_knobs("not json", &pages, &number_in_the_plugin).unwrap_err().starts_with("The built knob file could not be read"));
    }
}
