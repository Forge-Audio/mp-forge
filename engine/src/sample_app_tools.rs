//! The complete list of things the AI can do while making a Sample app.
//!
//! As with a plugin there is no shell, no network, and no way to reach a device in this list, and it cannot be
//! extended by anything the AI says. Reading is confined to the app and to MP Forge's guide to the device;
//! writing is confined to the app's own code and notes. The app's code is run only by the safety check and by
//! a look at its screen, both in a program of their own, and both only after the code has been read.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::agent::{Illustration, ToolDefinition, ToolOutput, Toolbox};
use crate::attachments::is_attachment;
use crate::file_tools::{
    EDIT_PLUGIN_FILE, FileTools, LIST_PLUGIN_FILES, LIST_REFERENCE, READ_PLUGIN_FILE, READ_REFERENCE, SEARCH_FILES, WRITE_PLUGIN_FILE, in_words,
    input_of_a_search, input_of_a_write, input_of_an_edit, no_input, path_input,
};
use crate::gate::{APP_TRIAL_CHECK, Gate, Outcome};
use crate::sample_trial::{Trial, picture_of_the_last_trial};
use crate::workspace::{PluginWorkspace, ReferenceLibrary};

const LOOK_AT_SCREEN: &str = "look_at_screen";
const CHECK_PLUGIN: &str = "check_plugin";

const THE_SCREEN_AFTER_THE_CHECK: &str = "The app's screen as the check left it, after everything on the device had been used. Every dot is drawn three times over.";
const THE_SCREEN_AFTER_THE_TOUCHES: &str = "The app's screen half a second after the last touch. Every dot is drawn three times over.";
const A_LOOK_IS_NOT_A_CHECK: &str = "This is only a look at the screen. Run check_plugin before you finish.";

pub struct SampleAppTools<'a> {
    files: FileTools,
    gate: &'a Gate,
}

impl<'a> SampleAppTools<'a> {
    pub fn open(app_dir: &Path, gate: &'a Gate) -> io::Result<Self> {
        let files = FileTools { plugin: PluginWorkspace::open(app_dir)?, reference: ReferenceLibrary::for_sample_apps() };
        Ok(Self { files, gate })
    }

    /// The safety check's report, with the screen the app ended on when it got as far as being played.
    fn check(&self) -> ToolOutput {
        let report = self.gate.inspect(self.files.root());
        let was_played = !matches!(report.outcome_of(APP_TRIAL_CHECK), None | Some(Outcome::NotRun));
        let screen = was_played.then(|| picture_of_the_last_trial(self.files.root())).flatten();
        let seen = screen.map(|png| Illustration { caption: THE_SCREEN_AFTER_THE_CHECK.to_string(), png });
        ToolOutput::success(report.to_string()).illustrated_with(seen.into_iter().collect())
    }

    /// Builds the app, does to it what was asked, and shows the screen it then has.
    fn look_at_screen(&self, input: &Value) -> ToolOutput {
        let touches = input.get("touches").cloned().unwrap_or_else(|| json!([]));
        match self.gate.try_sample_app(self.files.root(), &touches) {
            Ok(trial) => ToolOutput::success(what_a_look_says(&trial)).illustrated_with(vec![Illustration { caption: THE_SCREEN_AFTER_THE_TOUCHES.to_string(), png: trial.picture_as_png }]),
            Err(why_not) => ToolOutput::failure(why_not.join("\n")),
        }
    }
}

fn what_a_look_says(trial: &Trial) -> String {
    let found: Vec<String> = trial.problems().into_iter().chain(trial.warnings()).collect();
    std::iter::once(A_LOOK_IS_NOT_A_CHECK.to_string()).chain(found).collect::<Vec<_>>().join("\n")
}

impl Toolbox for SampleAppTools<'_> {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: LIST_REFERENCE,
                description: "Lists MP Forge's guide to the MPC Sample and the example apps. Call this once at the start to see what there is to read.",
                input_schema: no_input(),
            },
            ToolDefinition {
                name: READ_REFERENCE,
                description: "Reads one file from the reference library. Read mp-forge/mpc-sample.md and the example app nearest to what was asked for before you write anything. The library is read-only.",
                input_schema: path_input("A path from list_reference, such as mp-forge/mpc-sample.md"),
            },
            ToolDefinition {
                name: LIST_PLUGIN_FILES,
                description: "Lists the files of the app you are working on. Call this first to see the starter files MP Forge created.",
                input_schema: no_input(),
            },
            ToolDefinition {
                name: READ_PLUGIN_FILE,
                description: "Reads one of the app's files. Read src/mpc_sample_app.h, MP Forge's kit, before anything else, and read a file \
                    before you rewrite it. A picture the musician attached (in attachments/) comes back as an image you can look at.",
                input_schema: path_input("A path from list_plugin_files, such as src/App.cpp"),
            },
            ToolDefinition {
                name: WRITE_PLUGIN_FILE,
                description: "Creates or replaces one of the app's files with the full contents you give. You can write .cpp, .h or .hpp files \
                    inside src/, README.md and DESIGN.md. plugin.json is fixed, the kit is written again by MP Forge before every build, and \
                    nothing outside the app can be written. To change a small part of a file that is already there, use edit_plugin_file instead.",
                input_schema: input_of_a_write(),
            },
            ToolDefinition {
                name: EDIT_PLUGIN_FILE,
                description: "Changes one passage of a file that is already there, leaving the rest as it is. Quicker and safer than writing \
                    the whole file again. Give the passage exactly as it stands in the file. It has to be there only once, unless you set \
                    `everywhere` to change every place it is found. The same files can be changed as can be written.",
                input_schema: input_of_an_edit(),
            },
            ToolDefinition {
                name: SEARCH_FILES,
                description: "Finds every line that holds some words, in the app's own files or in the reference library, whatever their \
                    capitals. The answer names each file and line.",
                input_schema: input_of_a_search(),
            },
            ToolDefinition {
                name: LOOK_AT_SCREEN,
                description: "Builds the app, does to it the touches you list one after another, and shows you its screen half a second after \
                    the last one. Use it to see each state the screen has. mp-forge/mpc-sample.md says how a touch is written. With no \
                    touches, everything on the device is used in turn, as in the check.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "touches": {
                            "type": "array",
                            "description": "What is done on the device or on a MIDI keyboard plugged into it, in order, such as {\"control\": \"pad\", \"number\": 5, \"value\": 127} or {\"control\": \"note\", \"number\": 60, \"value\": 100}",
                            "items": { "type": "object" }
                        }
                    },
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: CHECK_PLUGIN,
                description: "Reads the app's code for anything an app may not do, builds it, and plays it through everything on the device. \
                    Returns a report with PASS, WARN or BLOCK for each step, and the screen the app ended on. An app is finished only when \
                    nothing is blocked.",
                input_schema: no_input(),
            },
        ]
    }

    fn run(&mut self, tool_name: &str, input: &Value) -> ToolOutput {
        let path = input["path"].as_str().unwrap_or_default();
        match tool_name {
            CHECK_PLUGIN => self.check(),
            LOOK_AT_SCREEN => self.look_at_screen(input),
            READ_PLUGIN_FILE if is_attachment(Path::new(path)) => self.files.show_attachment(path),
            for_a_file => in_words(self.files.attempt(for_a_file, input)),
        }
    }
}
