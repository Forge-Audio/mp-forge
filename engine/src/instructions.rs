//! What the AI is told before it starts work on a plugin.
//!
//! The text lives in `directives/` as plain documents so it can be read and edited without touching
//! code. It is built into the program, so the AI always gets the version this build was tested with.

const HOW_THE_WORKBENCH_WORKS: &str = include_str!("../../directives/plugin-maker.md");
const SOUND_DESIGN_RULES: &str = include_str!("../../directives/plugin-sound-design.md");
const SCREEN_DESIGN_RULES: &str = include_str!("../../directives/plugin-screen-design.md");
const WHAT_TO_SAY_AT_THE_END: &str = include_str!("../../directives/plugin-final-message.md");

pub fn for_making_plugins() -> String {
    [HOW_THE_WORKBENCH_WORKS, SOUND_DESIGN_RULES, SCREEN_DESIGN_RULES, WHAT_TO_SAY_AT_THE_END].join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instructions_cover_both_the_working_rules_and_the_screen_design() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("check_plugin"));
        assert!(instructions.contains("## The house style"));
    }

    #[test]
    fn the_instructions_ask_for_a_screen_of_its_own_and_say_how_to_look_at_it() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("Make the screen this plugin's own"));
        assert!(instructions.contains("`look_at_screen` shows you the screen as it stands"));
        assert!(instructions.contains("Put it in `DESIGN.md` before you style anything"));
        assert!(instructions.contains("list_fonts") && instructions.contains("edit_plugin_file") && instructions.contains("search_files"));
    }

    #[test]
    fn the_instructions_ask_for_a_sound_of_its_own_that_is_clean_and_moves() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("Make the sound this plugin's own"));
        assert!(instructions.contains("Put it in `DESIGN.md` under a heading `The sound`"));
        assert!(instructions.contains("Smooth everything that is heard"));
        assert!(instructions.contains("Up to four routes, one in each column: `Source`, `To`, `Amount`"));
        assert!(instructions.contains("You cannot listen to the sound"));
    }

    #[test]
    fn the_sound_comes_before_the_screen() {
        let instructions = for_making_plugins();

        let sound_starts_at = instructions.find("# Plugin sound design rules").unwrap();
        let screen_starts_at = instructions.find("# Plugin screen design rules").unwrap();

        assert!(sound_starts_at < screen_starts_at);
    }

    #[test]
    fn the_instructions_go_through_every_part_of_a_look() {
        let instructions = for_making_plugins();

        for part in ["### Lettering", "### Knobs", "### Faders, switches and choices", "### Backgrounds", "### Arrangement", "### Colour", "### Movement"] {
            assert!(instructions.contains(part), "{part}");
        }
        assert!(instructions.contains("Make it unlike the musician's other plugins"));
        assert!(instructions.contains("The screen is half of the work"));
    }

    #[test]
    fn our_own_notes_on_where_the_rules_come_from_are_not_sent() {
        let instructions = for_making_plugins();

        assert!(!instructions.contains("Where these rules come from"));
        assert!(!instructions.contains("Last checked"));
    }

    #[test]
    fn what_to_say_to_the_musician_comes_last_and_is_held_to_three_sentences() {
        let instructions = for_making_plugins();

        let said_at = instructions.find("# What you say to the musician").unwrap();
        let screen_rules_at = instructions.find("# Plugin screen design rules").unwrap();

        assert!(said_at > screen_rules_at);
        assert_eq!(instructions.matches("What you say to the musician").count(), 1);
        assert!(instructions.contains("at most three short sentences"));
        assert!(instructions.contains("count the sentences"));
    }

    #[test]
    fn the_instructions_say_how_to_put_pictures_on_the_screen() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("pictures.json") && instructions.contains("pictures/<name>.svg"));
        assert!(instructions.contains("read `mp-forge/pictures.md` in the reference library"));
        assert!(!instructions.contains("turnPictureClock"), "the picture clock's code is read from the library when it is needed");
    }

    #[test]
    fn the_instructions_tell_the_ai_to_look_at_the_pictures_it_is_shown() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("shows you each picture once it can"));
        assert!(!instructions.contains("You cannot see the result"));
    }

    #[test]
    fn the_instructions_give_the_real_measurements_of_a_page_and_cover_attached_pictures() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("A slot is 278 wide and only 107 tall"));
        assert!(instructions.contains("shows you every page of its screen"));
        assert!(instructions.contains("make your final message one short question"));
        assert!(instructions.contains("Each line starts with `[option]`"));
        assert!(instructions.contains("attachments/"));
    }

    #[test]
    fn the_instructions_say_how_to_style_the_screen_without_undoing_the_musicians_own_styling() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("look.json") && instructions.contains("`style_screen` is the only way to change it"));
        assert!(instructions.contains("Do not undo or redo their styling unless they ask for that"));
        assert!(instructions.contains("The hardware knobs follow that order"));
    }

    #[test]
    fn the_instructions_ask_for_a_default_preset_first_and_leave_older_plugins_presets_in_order() {
        let instructions = for_making_plugins();

        assert!(instructions.contains("A first preset named `Default`, which holds every control at its starting value"));
        assert!(instructions.contains("`addProgram(\"Default\", { ... })`"));
        assert!(instructions.contains("add `Default` at the front only when the musician asks for it"));
    }

    #[test]
    fn the_instructions_never_change_between_requests() {
        assert_eq!(for_making_plugins(), for_making_plugins());
    }
}
