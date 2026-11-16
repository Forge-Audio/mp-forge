//! How the musician laid out the inside of one control by hand: where its knob, its name and its value
//! sit, and how the name and value are lettered.
//!
//! Everything is measured from the control's own top left corner, in the MPC's pixels. The lettering is
//! drawn by the MPC itself, so only the fonts the MPC has can be asked for.

use serde_json::{Map, Value, json};

use crate::layers::Colour;
use crate::screen_layout::Area;

/// The fonts seen working on a real MPC.
pub const MPC_FONTS: [&str; 3] = ["Titillium Web", "Roboto", "Nexa"];
const SMALLEST_LETTERS: u32 = 8;
const TALLEST_LETTERS: u32 = 200;
/// A part may sit anywhere a page reaches, measured from its control's corner.
const FURTHEST_PART: u64 = 1280;
const PARTS_FIELDS: [&str; 3] = ["art", "name", "value"];
const LABEL_FIELDS: [&str; 7] = ["area", "font", "bold", "height", "colour", "align", "hidden"];
const AREA_FIELDS: [&str; 4] = ["x", "y", "width", "height"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

impl Align {
    const ALL: [Align; 3] = [Align::Left, Align::Centre, Align::Right];

    pub fn word(&self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Centre => "centre",
            Align::Right => "right",
        }
    }
}

/// One line of lettering the MPC draws: a control's name or its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelLook {
    pub area: Area,
    pub font: String,
    pub bold: bool,
    /// The height the MPC is asked to letter at.
    pub height: u32,
    pub colour: Colour,
    pub align: Align,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parts {
    /// Where the knob, fader or switch itself is drawn, and how big.
    pub art: Option<Area>,
    pub name: Option<LabelLook>,
    pub value: Option<LabelLook>,
}

impl Parts {
    /// The size of the smallest box, from the control's corner, that holds every part that shows.
    pub fn size(&self) -> Option<(u32, u32)> {
        let shown = |label: &Option<LabelLook>| label.as_ref().filter(|label| !label.hidden).map(|label| label.area);
        let areas: Vec<Area> = [self.art, shown(&self.name), shown(&self.value)].into_iter().flatten().collect();
        let right = areas.iter().map(Area::right).max()?;
        let bottom = areas.iter().map(Area::bottom).max()?;
        Some((right, bottom))
    }
}

pub fn parts_from(parts: &Value) -> Result<Parts, String> {
    let fields = known_fields(parts, &PARTS_FIELDS, "its \"parts\"")?;
    let label = |name: &str| fields.get(name).map(|label| label_from(label, name)).transpose();
    let art = fields.get("art").map(|art| area_from(art, "art")).transpose()?;
    let parts = Parts { art, name: label("name")?, value: label("value")? };
    if parts.size().is_none() { Err("its \"parts\" leave nothing of it showing.".to_string()) } else { Ok(parts) }
}

pub fn parts_as_json(parts: &Parts) -> Value {
    let mut written = json!({});
    if let Some(art) = &parts.art {
        written["art"] = area_as_json(art);
    }
    for (name, label) in [("name", &parts.name), ("value", &parts.value)] {
        if let Some(label) = label {
            written[name] = label_as_json(label);
        }
    }
    written
}

fn label_from(label: &Value, part: &str) -> Result<LabelLook, String> {
    let about = |problem: &str| format!("its {part}'s {problem}");
    let fields = known_fields(label, &LABEL_FIELDS, &format!("its {part}"))?;
    let text = |field: &str| fields.get(field).and_then(Value::as_str);
    let font = text("font").filter(|font| MPC_FONTS.contains(font));
    let height = fields.get("height").and_then(Value::as_u64).filter(|height| (u64::from(SMALLEST_LETTERS)..=u64::from(TALLEST_LETTERS)).contains(height));
    let align = text("align").and_then(|word| Align::ALL.into_iter().find(|align| align.word() == word));
    Ok(LabelLook {
        area: area_from(fields.get("area").unwrap_or(&Value::Null), part)?,
        font: font.ok_or_else(|| about(&format!("\"font\" is one the MPC has: {}.", MPC_FONTS.join(", "))))?.to_string(),
        bold: fields.get("bold") == Some(&Value::Bool(true)),
        height: height.ok_or_else(|| about(&format!("\"height\" is from {SMALLEST_LETTERS} to {TALLEST_LETTERS} pixels.")))? as u32,
        colour: text("colour").and_then(Colour::written_as).ok_or_else(|| about("\"colour\" is written like #1e3a28."))?,
        align: align.ok_or_else(|| about("\"align\" is \"left\", \"centre\" or \"right\"."))?,
        hidden: fields.get("hidden") == Some(&Value::Bool(true)),
    })
}

fn area_from(area: &Value, part: &str) -> Result<Area, String> {
    let wrong = || format!("its {part}'s area is whole pixels from the control's corner, such as {{\"x\": 0, \"y\": 0, \"width\": 90, \"height\": 90}}.");
    let fields = known_fields(area, &AREA_FIELDS, &format!("its {part}'s area")).map_err(|_| wrong())?;
    let pixels = |field: &str, least: u64| fields.get(field).and_then(Value::as_u64).filter(|pixels| (least..=FURTHEST_PART).contains(pixels)).map(|pixels| pixels as u32);
    match (pixels("x", 0), pixels("y", 0), pixels("width", 1), pixels("height", 1)) {
        (Some(x), Some(y), Some(width), Some(height)) => Ok(Area { x, y, width, height }),
        _ => Err(wrong()),
    }
}

fn known_fields<'a>(written: &'a Value, known: &[&str], what: &str) -> Result<&'a Map<String, Value>, String> {
    let fields = written.as_object().ok_or_else(|| format!("{what} is written between curly brackets."))?;
    match fields.keys().find(|key| !known.contains(&key.as_str())) {
        Some(stranger) => Err(format!("{what} has no \"{stranger}\". It has: {}.", known.join(", "))),
        None => Ok(fields),
    }
}

fn area_as_json(area: &Area) -> Value {
    json!({ "x": area.x, "y": area.y, "width": area.width, "height": area.height })
}

fn label_as_json(label: &LabelLook) -> Value {
    let mut written = json!({ "area": area_as_json(&label.area), "font": label.font, "height": label.height, "colour": label.colour.written(), "align": label.align.word() });
    if label.bold {
        written["bold"] = json!(true);
    }
    if label.hidden {
        written["hidden"] = json!(true);
    }
    written
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    pub fn label_over(area: Area) -> LabelLook {
        LabelLook { area, font: "Titillium Web".to_string(), bold: false, height: 26, colour: Colour { red: 255, green: 255, blue: 255 }, align: Align::Left, hidden: false }
    }

    /// A knob 120 pixels across with its name and then its value centred underneath.
    pub fn knob_over_its_name_and_value() -> Parts {
        let under = |y: u32| LabelLook { align: Align::Centre, ..label_over(Area { x: 0, y, width: 120, height: 30 }) };
        Parts { art: Some(Area { x: 0, y: 0, width: 120, height: 120 }), name: Some(under(120)), value: Some(under(150)) }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{knob_over_its_name_and_value, label_over};
    use super::*;

    const KNOB_OVER_NAME: &str = r##"{
        "art": { "x": 0, "y": 0, "width": 120, "height": 120 },
        "name": { "area": { "x": 0, "y": 120, "width": 120, "height": 30 }, "font": "Roboto", "bold": true, "height": 28, "colour": "#ff8800", "align": "centre" },
        "value": { "area": { "x": 0, "y": 150, "width": 120, "height": 30 }, "font": "Titillium Web", "height": 24, "colour": "#ffffff", "align": "right", "hidden": true }
    }"##;

    fn refusal_of(parts: &str) -> String {
        parts_from(&serde_json::from_str(parts).unwrap()).unwrap_err()
    }

    #[test]
    fn parts_are_read_with_where_they_sit_and_how_they_are_lettered() {
        let parts = parts_from(&serde_json::from_str(KNOB_OVER_NAME).unwrap()).unwrap();

        let name = parts.name.clone().unwrap();
        assert_eq!(parts.art, Some(Area { x: 0, y: 0, width: 120, height: 120 }));
        assert_eq!((name.font.as_str(), name.bold, name.height, name.align), ("Roboto", true, 28, Align::Centre));
        assert_eq!(name.colour, Colour { red: 0xff, green: 0x88, blue: 0 });
        assert!(parts.value.unwrap().hidden && !name.hidden);
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let parts = parts_from(&serde_json::from_str(KNOB_OVER_NAME).unwrap()).unwrap();

        assert_eq!(parts_from(&parts_as_json(&parts)), Ok(parts));
    }

    #[test]
    fn a_control_is_as_big_as_the_parts_of_it_that_show() {
        let all_showing = knob_over_its_name_and_value();
        let value_hidden = Parts { value: all_showing.value.clone().map(|value| LabelLook { hidden: true, ..value }), ..all_showing.clone() };

        assert_eq!(all_showing.size(), Some((120, 180)));
        assert_eq!(value_hidden.size(), Some((120, 150)));
        assert_eq!(Parts::default().size(), None);
    }

    #[test]
    fn only_lettering_the_mpc_can_draw_is_accepted() {
        let label = |changed: &str| format!(r##"{{ "name": {{ "area": {{ "x": 0, "y": 0, "width": 100, "height": 30 }}, "font": "Roboto", "height": 26, "colour": "#ffffff", "align": "left"{changed} }} }}"##);

        assert!(parts_from(&serde_json::from_str(&label("")).unwrap()).is_ok());
        assert!(refusal_of(&label(r#", "font": "Comic Sans""#)).contains("one the MPC has: Titillium Web, Roboto, Nexa"));
        assert!(refusal_of(&label(r#", "height": 4"#)).contains("from 8 to 200"));
        assert!(refusal_of(&label(r#", "align": "justified""#)).contains("\"left\", \"centre\" or \"right\""));
        assert!(refusal_of(&label(r#", "colour": "white""#)).contains("written like #1e3a28"));
        assert!(refusal_of(&label(r#", "glow": true"#)).contains("its name has no \"glow\""));
    }

    #[test]
    fn parts_that_are_misplaced_or_show_nothing_are_refused() {
        assert!(refusal_of(r#"{ "art": { "x": -4, "y": 0, "width": 90, "height": 90 } }"#).contains("whole pixels from the control's corner"));
        assert!(refusal_of(r#"{ "art": { "x": 0, "y": 0, "width": 0, "height": 90 } }"#).contains("whole pixels"));
        assert!(refusal_of(r#"{ "knob": {} }"#).contains("has no \"knob\""));
        assert!(refusal_of("{}").contains("leave nothing of it showing"));
        assert_eq!(label_over(Area { x: 0, y: 0, width: 1, height: 1 }).height, 26);
    }
}
