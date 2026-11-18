//! Which of the MPC's sixteen hardware knobs turns each control on a page.
//!
//! The screen's own layout gives every slot the knob that sits beside it on the MPC. The musician can give
//! a control another knob, and the look keeps those choices. Their choices come first. A control they left
//! alone keeps its own knob while that is free, and takes a free one otherwise.

use crate::screen_layout::SLOTS_PER_SIDE;

pub const KNOBS_ON_A_PAGE: u32 = 16;
const TOP_LEFT_KNOB: u32 = 13;

/// A knob the musician gave to a control, by the control's name in `ui.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnobChoice {
    pub control: String,
    pub knob: u32,
}

/// A control on a page: its name, and the slot the layout gives it, counted in reading order from zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seat<'a> {
    pub control: &'a str,
    pub slot: usize,
}

/// The knob beside a slot. The MPC counts its knobs from the bottom left, and a page's slots run from the top left.
pub fn own_knob_of(slot: usize) -> u32 {
    let slot = slot as u32;
    TOP_LEFT_KNOB + slot % SLOTS_PER_SIDE - SLOTS_PER_SIDE * (slot / SLOTS_PER_SIDE)
}

/// The column of four knobs the MPC takes together, counted from zero at the left.
pub fn column_of_knob(knob: u32) -> usize {
    ((knob - 1) % SLOTS_PER_SIDE) as usize
}

/// The knob that turns each control, in the order the controls were given. A control beyond the sixteenth gets none.
pub fn knobs_for(seats: &[Seat], chosen: &[KnobChoice]) -> Vec<Option<u32>> {
    let mut handed_out = HandedOut { knobs: vec![None; seats.len()] };
    for (position, seat) in seats.iter().enumerate() {
        let wanted = chosen.iter().find(|choice| choice.control == seat.control).map(|choice| choice.knob);
        handed_out.give_if_free(position, wanted);
    }
    for (position, seat) in seats.iter().enumerate() {
        handed_out.give_if_free(position, Some(own_knob_of(seat.slot)));
    }
    for position in 0..seats.len() {
        let first_free = knobs_in_reading_order().find(|knob| handed_out.is_free(*knob));
        handed_out.give_if_free(position, first_free);
    }
    handed_out.knobs
}

fn knobs_in_reading_order() -> impl Iterator<Item = u32> {
    (0..KNOBS_ON_A_PAGE as usize).map(own_knob_of)
}

/// The knobs given so far, one place for each control.
struct HandedOut {
    knobs: Vec<Option<u32>>,
}

impl HandedOut {
    fn is_free(&self, knob: u32) -> bool {
        !self.knobs.contains(&Some(knob))
    }

    /// A control that already has a knob keeps it, and a knob that is taken stays where it is.
    fn give_if_free(&mut self, position: usize, knob: Option<u32>) {
        let can_be_given = knob.is_some_and(|knob| (1..=KNOBS_ON_A_PAGE).contains(&knob) && self.is_free(knob));
        if self.knobs[position].is_none() && can_be_given {
            self.knobs[position] = knob;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seats<'a>(controls: &[&'a str]) -> Vec<Seat<'a>> {
        controls.iter().enumerate().map(|(slot, control)| Seat { control, slot }).collect()
    }

    fn chosen(choices: &[(&str, u32)]) -> Vec<KnobChoice> {
        choices.iter().map(|(control, knob)| KnobChoice { control: control.to_string(), knob: *knob }).collect()
    }

    #[test]
    fn the_knob_beside_a_slot_is_counted_from_the_bottom_left() {
        assert_eq!([own_knob_of(0), own_knob_of(3), own_knob_of(4), own_knob_of(12), own_knob_of(15)], [13, 16, 9, 1, 4]);
    }

    #[test]
    fn the_knobs_of_one_column_are_taken_together() {
        assert_eq!([column_of_knob(13), column_of_knob(9), column_of_knob(1), column_of_knob(16), column_of_knob(4)], [0, 0, 0, 3, 3]);
    }

    #[test]
    fn with_nothing_chosen_each_control_has_the_knob_beside_it() {
        assert_eq!(knobs_for(&seats(&["Time", "Mix", "Tone"]), &[]), [Some(13), Some(14), Some(15)]);
    }

    #[test]
    fn two_controls_that_swapped_knobs_each_turn_with_the_others() {
        let swapped = chosen(&[("Time", 14), ("Mix", 13)]);

        assert_eq!(knobs_for(&seats(&["Time", "Mix", "Tone"]), &swapped), [Some(14), Some(13), Some(15)]);
    }

    #[test]
    fn a_control_can_be_given_a_knob_nothing_sat_beside() {
        assert_eq!(knobs_for(&seats(&["Time", "Mix"]), &chosen(&[("Mix", 1)])), [Some(13), Some(1)]);
    }

    #[test]
    fn a_control_whose_own_knob_was_given_away_takes_the_first_free_one() {
        let time_has_mixes_knob = chosen(&[("Time", 14)]);

        assert_eq!(knobs_for(&seats(&["Time", "Mix", "Tone"]), &time_has_mixes_knob), [Some(14), Some(13), Some(15)]);
    }

    #[test]
    fn a_choice_for_a_control_the_page_no_longer_has_changes_nothing() {
        assert_eq!(knobs_for(&seats(&["Time", "Mix"]), &chosen(&[("Wobble", 13)])), [Some(13), Some(14)]);
    }

    #[test]
    fn a_control_on_a_page_twice_has_its_chosen_knob_once() {
        let knobs = knobs_for(&seats(&["Mix", "Mix"]), &chosen(&[("Mix", 5)]));

        assert_eq!(knobs, [Some(5), Some(14)]);
    }
}
