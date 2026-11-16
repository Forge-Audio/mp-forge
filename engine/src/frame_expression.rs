//! The small sums a drawing may contain, such as `{360 * t}`, worked out afresh for every frame.
//!
//! A sum can use numbers, `+ - * /`, brackets, the names `t`, `frame` and `frames`, and a few
//! functions. `sin` and `cos` count in whole turns, so `sin(t)` ends exactly where it began.

use std::f64::consts::TAU;

const DECIMAL_PLACES: usize = 3;

/// One frame of a picture: which frame it is, and how far through the movement that is, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    pub frame: u32,
    pub frames: u32,
    pub progress: f64,
}

/// Returns the drawing with every `{sum}` replaced by its value at this moment.
pub fn fill_in(drawing: &str, moment: &Moment) -> Result<String, String> {
    let mut filled = String::with_capacity(drawing.len());
    let mut rest = drawing;
    while let Some(opening) = rest.find('{') {
        let after_opening = &rest[opening + 1..];
        let closing = after_opening.find('}').ok_or("A `{` in the drawing is never closed with `}`.")?;
        filled.push_str(&rest[..opening]);
        filled.push_str(&written(value_of(&after_opening[..closing], moment)?));
        rest = &after_opening[closing + 1..];
    }
    filled.push_str(rest);
    Ok(filled)
}

pub fn value_of(sum: &str, moment: &Moment) -> Result<f64, String> {
    let mut reader = Reader { rest: sum, moment };
    let explain = |problem: String| format!("`{{{sum}}}` cannot be worked out: {problem}");
    let value = reader.sum().map_err(explain)?;
    if !reader.is_finished() {
        return Err(explain(format!("`{}` was not expected", reader.rest.trim())));
    }
    if value.is_finite() { Ok(value) } else { Err(explain("it does not come to a number".to_string())) }
}

fn written(value: f64) -> String {
    let rounded = format!("{value:.DECIMAL_PLACES$}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" { "0".to_string() } else { trimmed.to_string() }
}

struct Reader<'a> {
    rest: &'a str,
    moment: &'a Moment,
}

impl<'a> Reader<'a> {
    fn sum(&mut self) -> Result<f64, String> {
        let mut total = self.product()?;
        loop {
            if self.takes('+') {
                total += self.product()?;
            } else if self.takes('-') {
                total -= self.product()?;
            } else {
                return Ok(total);
            }
        }
    }

    fn product(&mut self) -> Result<f64, String> {
        let mut total = self.signed()?;
        loop {
            if self.takes('*') {
                total *= self.signed()?;
            } else if self.takes('/') {
                total /= self.signed()?;
            } else {
                return Ok(total);
            }
        }
    }

    fn signed(&mut self) -> Result<f64, String> {
        if self.takes('-') { Ok(-self.signed()?) } else { self.single() }
    }

    fn single(&mut self) -> Result<f64, String> {
        if self.takes('(') {
            return self.rest_of_bracket();
        }
        let word = self.word();
        match word.chars().next() {
            None => Err("a number or a name is missing".to_string()),
            Some(first) if first.is_ascii_digit() || first == '.' => word.parse().map_err(|_| format!("`{word}` is not a number")),
            Some(_) if self.takes('(') => apply(word, &self.arguments()?),
            Some(_) => self.named(word),
        }
    }

    fn rest_of_bracket(&mut self) -> Result<f64, String> {
        let value = self.sum()?;
        if self.takes(')') { Ok(value) } else { Err("a `)` is missing".to_string()) }
    }

    fn arguments(&mut self) -> Result<Vec<f64>, String> {
        let mut arguments = vec![self.sum()?];
        while self.takes(',') {
            arguments.push(self.sum()?);
        }
        if self.takes(')') { Ok(arguments) } else { Err("a `)` is missing".to_string()) }
    }

    fn named(&self, name: &str) -> Result<f64, String> {
        match name {
            "t" => Ok(self.moment.progress),
            "frame" => Ok(f64::from(self.moment.frame)),
            "frames" => Ok(f64::from(self.moment.frames)),
            unknown => Err(format!("`{unknown}` is not a name it knows; use t, frame or frames")),
        }
    }

    /// The run of letters, digits and dots at the front, such as `360`, `0.5`, `t` or `sin`.
    fn word(&mut self) -> &'a str {
        let from_first_letter = self.rest.trim_start();
        let length = from_first_letter.find(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '.')).unwrap_or(from_first_letter.len());
        let (word, rest) = from_first_letter.split_at(length);
        self.rest = rest;
        word
    }

    fn takes(&mut self, sign: char) -> bool {
        match self.rest.trim_start().strip_prefix(sign) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn is_finished(&self) -> bool {
        self.rest.trim().is_empty()
    }
}

fn apply(function: &str, arguments: &[f64]) -> Result<f64, String> {
    match (function, arguments) {
        ("sin", [turns]) => Ok((turns * TAU).sin()),
        ("cos", [turns]) => Ok((turns * TAU).cos()),
        ("abs", [value]) => Ok(value.abs()),
        ("round", [value]) => Ok(value.round()),
        ("floor", [value]) => Ok(value.floor()),
        ("min", [one, other]) => Ok(one.min(*other)),
        ("max", [one, other]) => Ok(one.max(*other)),
        ("sin" | "cos" | "abs" | "round" | "floor", _) => Err(format!("`{function}` takes one value")),
        ("min" | "max", _) => Err(format!("`{function}` takes two values")),
        (unknown, _) => Err(format!("`{unknown}` is not a function it knows; use sin, cos, abs, round, floor, min or max")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUARTER_WAY: Moment = Moment { frame: 6, frames: 24, progress: 0.25 };

    #[test]
    fn sums_follow_the_usual_order_of_arithmetic() {
        assert_eq!(value_of("2 + 3 * 4", &QUARTER_WAY).unwrap(), 14.0);
        assert_eq!(value_of("(2 + 3) * 4", &QUARTER_WAY).unwrap(), 20.0);
        assert_eq!(value_of("-10 + 40 * t", &QUARTER_WAY).unwrap(), 0.0);
        assert_eq!(value_of("frame / frames", &QUARTER_WAY).unwrap(), 0.25);
    }

    #[test]
    fn sin_and_cos_count_in_whole_turns() {
        assert!((value_of("sin(t)", &QUARTER_WAY).unwrap() - 1.0).abs() < 1e-9);
        assert!(value_of("cos(t)", &QUARTER_WAY).unwrap().abs() < 1e-9);
        assert_eq!(value_of("max(2, min(9, 5))", &QUARTER_WAY).unwrap(), 5.0);
    }

    #[test]
    fn every_sum_in_a_drawing_is_replaced_by_its_value() {
        let filled = fill_in(r#"<g transform="rotate({360 * t} 50 50)" opacity="{1 - t / 2}"/>"#, &QUARTER_WAY).unwrap();

        assert_eq!(filled, r#"<g transform="rotate(90 50 50)" opacity="0.875"/>"#);
    }

    #[test]
    fn a_drawing_without_sums_is_left_as_it_is() {
        assert_eq!(fill_in("<svg/>", &QUARTER_WAY).unwrap(), "<svg/>");
    }

    #[test]
    fn a_sum_that_cannot_be_worked_out_says_which_and_why() {
        let unknown_name = fill_in("{speed * 2}", &QUARTER_WAY).unwrap_err();
        let divided_by_nothing = fill_in("{1 / (t - t)}", &QUARTER_WAY).unwrap_err();

        assert!(unknown_name.contains("{speed * 2}") && unknown_name.contains("use t, frame or frames"), "{unknown_name}");
        assert!(divided_by_nothing.contains("does not come to a number"), "{divided_by_nothing}");
        assert!(fill_in("{2 +}", &QUARTER_WAY).is_err());
        assert!(fill_in("{2 3}", &QUARTER_WAY).is_err());
        assert!(fill_in("{sin(1, 2)}", &QUARTER_WAY).is_err());
        assert!(fill_in("rotate({360 * t", &QUARTER_WAY).is_err());
    }
}
