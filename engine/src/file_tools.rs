//! The tools that read and write files, which the AI has whatever it is making: a plugin or a Sample app.
//!
//! Reading is confined to what it is working on and to a reference library. Writing is confined to the kinds
//! of file its work is made of. Every path it names is checked by the workspace.

use std::fmt::Display;
use std::path::Path;

use serde_json::{Value, json};

use crate::agent::{Illustration, ToolOutput};
use crate::file_text::{lines_holding, with_passage_replaced};
use crate::workspace::{PluginWorkspace, ReferenceLibrary};

pub(crate) const LIST_REFERENCE: &str = "list_reference";
pub(crate) const READ_REFERENCE: &str = "read_reference";
pub(crate) const LIST_PLUGIN_FILES: &str = "list_plugin_files";
pub(crate) const READ_PLUGIN_FILE: &str = "read_plugin_file";
pub(crate) const WRITE_PLUGIN_FILE: &str = "write_plugin_file";
pub(crate) const EDIT_PLUGIN_FILE: &str = "edit_plugin_file";
pub(crate) const SEARCH_FILES: &str = "search_files";

const SEARCH_THE_PLUGIN: &str = "plugin";
const SEARCH_THE_REFERENCE: &str = "reference";

pub(crate) struct FileTools {
    pub(crate) plugin: PluginWorkspace,
    pub(crate) reference: ReferenceLibrary,
}

impl FileTools {
    pub(crate) fn root(&self) -> &Path {
        self.plugin.root()
    }

    /// Does what one of the file tools was asked to, and answers in words.
    pub(crate) fn attempt(&self, tool_name: &str, input: &Value) -> Result<String, String> {
        match tool_name {
            LIST_REFERENCE => self.reference.list().map(as_lines).map_err(explain),
            READ_REFERENCE => self.reference.read(text_field(input, "path")?).map_err(explain),
            LIST_PLUGIN_FILES => self.plugin.list().map(as_lines).map_err(explain),
            READ_PLUGIN_FILE => self.plugin.read(text_field(input, "path")?).map_err(explain),
            WRITE_PLUGIN_FILE => self.write_file(input),
            EDIT_PLUGIN_FILE => self.edit_file(input),
            SEARCH_FILES => self.search_files(input),
            unknown => Err(format!("There is no tool called `{unknown}`.")),
        }
    }

    fn write_file(&self, input: &Value) -> Result<String, String> {
        let path = text_field(input, "path")?;
        let contents = text_field(input, "contents")?;
        self.plugin.write(path, contents).map_err(explain)?;
        Ok(format!("Wrote {path} ({} bytes).", contents.len()))
    }

    /// Swaps one passage of a file for another, under the same rules as writing the whole file.
    fn edit_file(&self, input: &Value) -> Result<String, String> {
        let path = text_field(input, "path")?;
        let everywhere = input["everywhere"].as_bool().unwrap_or(false);
        let before = self.plugin.read(path).map_err(explain)?;
        let (after, places) = with_passage_replaced(&before, text_field(input, "find")?, text_field(input, "replace")?, everywhere)?;
        self.plugin.write(path, &after).map_err(explain)?;
        Ok(format!("Changed {path} in {places} {}.", if places == 1 { "place" } else { "places" }))
    }

    fn search_files(&self, input: &Value) -> Result<String, String> {
        let wanted = text_field(input, "text")?;
        match text_field(input, "in")? {
            SEARCH_THE_PLUGIN => Ok(lines_holding(wanted, texts_of(self.plugin.list().map_err(explain)?, |path| self.plugin.read(path).ok()))),
            SEARCH_THE_REFERENCE => Ok(lines_holding(wanted, texts_of(self.reference.list().map_err(explain)?, |path| self.reference.read(path).ok()))),
            _ => Err(format!("`in` is \"{SEARCH_THE_PLUGIN}\" or \"{SEARCH_THE_REFERENCE}\".")),
        }
    }

    /// A picture the musician attached is handed back as a picture to look at, not as text.
    pub(crate) fn show_attachment(&self, path: &str) -> ToolOutput {
        match self.plugin.read_attachment(path) {
            Ok(png) => {
                let picture = Illustration { caption: format!("{path}, attached by the musician."), png };
                ToolOutput::success(format!("{path} is a picture. It follows.")).illustrated_with(vec![picture])
            }
            Err(problem) => ToolOutput::failure(explain(problem)),
        }
    }
}

/// The files that can be read as text, each with its text. A picture or anything else unreadable is passed over.
fn texts_of(paths: Vec<String>, read: impl Fn(&str) -> Option<String>) -> impl Iterator<Item = (String, String)> {
    paths.into_iter().filter_map(move |path| read(&path).map(|text| (path, text)))
}

pub(crate) fn in_words(outcome: Result<String, String>) -> ToolOutput {
    match outcome {
        Ok(content) => ToolOutput::success(content),
        Err(explanation) => ToolOutput::failure(explanation),
    }
}

// What each file tool takes, said the way a model is told.

pub(crate) fn no_input() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

pub(crate) fn path_input(description: &str) -> Value {
    json!({
        "type": "object",
        "properties": { "path": { "type": "string", "description": description } },
        "required": ["path"],
        "additionalProperties": false
    })
}

pub(crate) fn input_of_a_write() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Where to write, relative to the plugin folder" },
            "contents": { "type": "string", "description": "The complete new contents of the file" }
        },
        "required": ["path", "contents"],
        "additionalProperties": false
    })
}

pub(crate) fn input_of_an_edit() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "The file to change, relative to the plugin folder" },
            "find": { "type": "string", "description": "The passage as it stands now, copied exactly" },
            "replace": { "type": "string", "description": "What to put in its place" },
            "everywhere": { "type": "boolean", "description": "True to change every place the passage is found" }
        },
        "required": ["path", "find", "replace"],
        "additionalProperties": false
    })
}

pub(crate) fn input_of_a_search() -> Value {
    json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "The words to look for, such as a parameter's name or a function" },
            "in": { "type": "string", "enum": [SEARCH_THE_PLUGIN, SEARCH_THE_REFERENCE], "description": "Where to look" }
        },
        "required": ["text", "in"],
        "additionalProperties": false
    })
}

pub(crate) fn text_field<'a>(input: &'a Value, name: &str) -> Result<&'a str, String> {
    input.get(name).and_then(Value::as_str).ok_or_else(|| format!("This tool needs `{name}` as text."))
}

fn as_lines(paths: Vec<String>) -> String {
    paths.join("\n")
}

pub(crate) fn explain(error: impl Display) -> String {
    error.to_string()
}
