//! The complete list of things the AI can do while making a plugin.
//!
//! There is no shell, no network, and no way to reach an MPC in this list, and it cannot be extended
//! by anything the AI says. Reading is confined to the plugin and the reference library; writing is
//! confined to the plugin's own source, screen layout, pictures, design notes and README. The look the
//! musician styles by hand is changed only through the styling tool, one checked step at a time.
//!
//! Beyond reading and writing, the tools are there so the AI can design a screen well: a quick look at
//! the screen whenever it wants one, with close-ups, small edits and searches that make a second and a
//! third try cheap, and the list of fonts it can letter artwork in.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::agent::{Illustration, ToolDefinition, ToolOutput, Toolbox};
use crate::arranged_screen::arranged_screen_of;
use crate::artwork;
use crate::attachments::is_attachment;
use crate::file_tools::{
    EDIT_PLUGIN_FILE, FileTools, LIST_PLUGIN_FILES, LIST_REFERENCE, READ_PLUGIN_FILE, READ_REFERENCE, SEARCH_FILES, WRITE_PLUGIN_FILE, explain, in_words,
    input_of_a_search, input_of_a_write, input_of_an_edit, no_input, path_input,
};
use crate::fonts;
use crate::gate::{Gate, GateReport, Outcome, PICTURES_CHECK, SCREEN_CHECK};
use crate::kept_look::keep_if_it_fits;
use crate::look::{Look, look_of};
use crate::mcp_server;
use crate::page_art;
use crate::page_view::{CloseUp, close_up};
use crate::sample_app;
use crate::sample_app_tools::SampleAppTools;
use crate::picture_review::{HOW_TO_READ_THE_PAGES, HOW_TO_READ_THE_PICTURES, PictureReview};
use crate::screen_layout::Area;
use crate::sdk::Sdk;
use crate::styling;
use crate::workspace::{PluginWorkspace, ReferenceLibrary};

const STYLE_SCREEN: &str = "style_screen";
const LOOK_AT_SCREEN: &str = "look_at_screen";
const LIST_FONTS: &str = "list_fonts";
const CHECK_PLUGIN: &str = "check_plugin";

/// Every tool there is. An outside agent program is allowed these and nothing else.
pub const TOOL_NAMES: [&str; 11] = [
    LIST_REFERENCE,
    READ_REFERENCE,
    LIST_PLUGIN_FILES,
    READ_PLUGIN_FILE,
    WRITE_PLUGIN_FILE,
    EDIT_PLUGIN_FILE,
    SEARCH_FILES,
    STYLE_SCREEN,
    LOOK_AT_SCREEN,
    LIST_FONTS,
    CHECK_PLUGIN,
];

const MOST_FONTS_LISTED: usize = 400;
const A_QUICK_LOOK: &str = "This is a quick look. Nothing was built or tested, so run check_plugin before you finish. A control     the plugin has not been built with yet is drawn at a stand-in setting, and the lettering is in a stand-in font.";
const NOTHING_TO_LOOK_AT: &str = "There is no such page to look at. Give a page name from ui.json, or leave `page` out to see them all.";
const A_CLOSE_UP_NEEDS_ITS_PAGE: &str = "Name the `page` to look closer at.";

pub struct PluginTools<'a> {
    files: FileTools,
    gate: &'a Gate,
    review: PictureReview,
}

impl<'a> PluginTools<'a> {
    pub fn open(plugin_dir: &Path, sdk_root: &Path, gate: &'a Gate) -> io::Result<Self> {
        let files = FileTools { plugin: PluginWorkspace::open(plugin_dir)?, reference: ReferenceLibrary::open(sdk_root)? };
        Ok(Self { files, gate, review: PictureReview::default() })
    }

    fn attempt(&self, tool_name: &str, input: &Value) -> Result<String, String> {
        match tool_name {
            STYLE_SCREEN => self.style_screen(input),
            LIST_FONTS => Ok(fonts_on_this_computer()),
            for_a_file => self.files.attempt(for_a_file, input),
        }
    }

    /// Draws the pictures and the pages as they stand and shows them, without building or testing anything.
    /// With a part of one page named, that part is shown enlarged instead.
    fn look_at_screen(&mut self, input: &Value) -> ToolOutput {
        let plugin = self.files.root().to_path_buf();
        if let Err(problems) = artwork::draw_pictures(&plugin) {
            return ToolOutput::failure(problems.join(" "));
        }
        let seen = match (input["page"].as_str(), part_asked_for(input)) {
            (None, Some(_)) => return ToolOutput::failure(A_CLOSE_UP_NEEDS_ITS_PAGE),
            (Some(page), Some(part)) => close_up(&plugin, page, part).map(|seen| illustration_of_close_up(page, seen)).into_iter().collect(),
            (page, None) => self.review.show_pages(&plugin, page),
        };
        if seen.is_empty() { ToolOutput::failure(NOTHING_TO_LOOK_AT) } else { ToolOutput::success(A_QUICK_LOOK).illustrated_with(seen) }
    }

    /// Makes the changes asked for to the plugin's look and keeps it, unless the result would not fit
    /// the screen or could not be drawn. Either way the answer says where everything now sits.
    fn style_screen(&self, input: &Value) -> Result<String, String> {
        let changes = input.get("changes").and_then(Value::as_array).ok_or("This tool needs `changes` as a list. An empty list just shows where everything is.")?;
        let plugin = self.files.root();
        let look_before = look_of(plugin)?;
        if changes.is_empty() {
            return Ok(styling::described(&arranged_screen_of(plugin), &look_before));
        }
        let look = styling::restyled(&look_before, &arranged_screen_of(plugin), changes)?;
        refuse_art_that_cannot_be_drawn(plugin, &look)?;
        let screen = keep_if_it_fits(plugin, &look).map_err(explain)?;
        Ok(format!("Kept. The screen is now arranged like this. Run check_plugin to see it.\n\n{}", styling::described(&screen, &look)))
    }

    /// The safety check's report, with the pages and pictures the AI has not yet seen as they are now.
    fn check_plugin(&mut self) -> ToolOutput {
        let report = self.gate.inspect(self.files.root());
        let pages = if passed(&report, SCREEN_CHECK) { self.review.show_changed_pages(self.files.root()) } else { Vec::new() };
        let pictures = if passed(&report, PICTURES_CHECK) { self.review.show_what_changed(self.files.root()) } else { Vec::new() };
        let how_to_read = [(!pages.is_empty()).then_some(HOW_TO_READ_THE_PAGES), (!pictures.is_empty()).then_some(HOW_TO_READ_THE_PICTURES)];
        let words: Vec<String> = std::iter::once(report.to_string()).chain(how_to_read.into_iter().flatten().map(str::to_string)).collect();
        ToolOutput::success(words.join("\n\n")).illustrated_with(pages.into_iter().chain(pictures).collect())
    }
}

fn fonts_on_this_computer() -> String {
    let names = fonts::font_names();
    let listed: Vec<&str> = names.iter().map(String::as_str).take(MOST_FONTS_LISTED).collect();
    format!("Fonts for words drawn as artwork (the \"words\" layer of style_screen). A control's own name and value can only use Titillium Web, Roboto or Nexa.\n\n{}", listed.join("\n"))
}

/// The part of a page asked for as `area`, when all four of its measurements are there.
fn part_asked_for(input: &Value) -> Option<Area> {
    let measure = |name: &str| input["area"][name].as_u64().and_then(|pixels| u32::try_from(pixels).ok());
    Some(Area { x: measure("x")?, y: measure("y")?, width: measure("width")?, height: measure("height")? })
}

fn illustration_of_close_up(page: &str, seen: CloseUp) -> Illustration {
    let CloseUp { png, part, times } = seen;
    let caption = format!("Page `{page}` from ({}, {}), {} wide and {} tall, with each pixel drawn {times} times over.", part.x, part.y, part.width, part.height);
    Illustration { caption, png }
}

fn refuse_art_that_cannot_be_drawn(plugin: &Path, look: &Look) -> Result<(), String> {
    let problems: Vec<String> = look.pages.iter().flat_map(|page| page_art::problems_with(plugin, &page.page, &page.artwork)).collect();
    if problems.is_empty() { Ok(()) } else { Err(problems.join(" ")) }
}

/// A check that stopped early leaves the last check's images and build behind, which must not be shown as new.
fn passed(report: &GateReport, check_name: &str) -> bool {
    report.outcome_of(check_name) == Some(&Outcome::Passed)
}

/// What a tool request means, in words for the person watching.
pub fn describe_step(tool_name: &str, input: &Value) -> String {
    let path = input["path"].as_str().unwrap_or_default();
    match tool_name {
        LIST_REFERENCE => "looking at what Akai's documentation covers".to_string(),
        READ_REFERENCE => format!("reading {path}"),
        LIST_PLUGIN_FILES => "looking at the plugin's files".to_string(),
        READ_PLUGIN_FILE => format!("reading the plugin's {path}"),
        WRITE_PLUGIN_FILE => format!("writing {path}"),
        EDIT_PLUGIN_FILE => format!("writing a change to {path}"),
        SEARCH_FILES => format!("searching for \"{}\"", input["text"].as_str().unwrap_or_default()),
        STYLE_SCREEN => how_many_changes(input),
        LOOK_AT_SCREEN => how_it_looks(input),
        LIST_FONTS => "looking at the fonts on this computer".to_string(),
        CHECK_PLUGIN => "running the safety check (this builds and tests the plugin)".to_string(),
        other => other.to_string(),
    }
}

fn how_it_looks(input: &Value) -> String {
    match (input["page"].as_str(), input["area"].is_object()) {
        (Some(page), true) => format!("looking closely at part of the {page} page"),
        (Some(page), false) => format!("looking at the {page} page"),
        (None, _) => "looking at the whole screen".to_string(),
    }
}

fn how_many_changes(input: &Value) -> String {
    match input["changes"].as_array().map(Vec::len) {
        Some(0) | None => "looking at where everything sits on the screen".to_string(),
        Some(1) => "styling the screen (1 change)".to_string(),
        Some(count) => format!("styling the screen ({count} changes)"),
    }
}

/// Serves the tools for one plugin, or one Sample app, on standard input and output until the caller hangs up.
/// This is what an outside agent program, such as Claude Code, starts to reach the tools.
/// `online_plugins` says whether the musician has switched those on, which the safety check has to know.
pub fn serve_on_standard_io(sdk: &Sdk, plugin_dir: &Path, online_plugins: bool) -> io::Result<()> {
    let gate = Gate::new(sdk.clone()).with_online_plugins(online_plugins);
    if sample_app::is_one(plugin_dir) {
        return mcp_server::serve(&mut SampleAppTools::open(plugin_dir, &gate)?, io::stdin().lock(), io::stdout().lock());
    }
    let mut tools = PluginTools::open(plugin_dir, sdk.root(), &gate)?;
    mcp_server::serve(&mut tools, io::stdin().lock(), io::stdout().lock())
}

impl Toolbox for PluginTools<'_> {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: LIST_REFERENCE,
                description: "Lists MP Forge's own guides, Akai's documentation, the plugin framework headers and the example plugins. \
                    Call this once at the start to see what there is to read.",
                input_schema: no_input(),
            },
            ToolDefinition {
                name: READ_REFERENCE,
                description: "Reads one file from the reference library. Read the overview, the DSP cookbook, the \
                    framework header and the closest example before writing audio code, the UI guide before \
                    writing ui.json, mp-forge/pictures.md before drawing a picture, and mp-forge/online.md before bringing in sound from the internet. The library is read-only.",
                input_schema: path_input("A path from list_reference, such as docs/overview.md"),
            },
            ToolDefinition {
                name: LIST_PLUGIN_FILES,
                description: "Lists the files of the plugin you are working on. Call this first to see the starter \
                    files MP Forge created.",
                input_schema: no_input(),
            },
            ToolDefinition {
                name: READ_PLUGIN_FILE,
                description: "Reads one of the plugin's files. Read a file before you rewrite it. A picture the \
                    musician attached (in attachments/) comes back as an image you can look at.",
                input_schema: path_input("A path from list_plugin_files, such as src/DustyTape.cpp"),
            },
            ToolDefinition {
                name: WRITE_PLUGIN_FILE,
                description: "Creates or replaces one of the plugin's files with the full contents you give. \
                    You can write ui.json, pictures.json, sample-knobs.json, README.md, DESIGN.md, .cpp, .h or .hpp files inside src/, and .svg \
                    drawings inside pictures/. plugin.json is fixed and nothing outside the plugin can be written. \
                    To change a small part of a file that is already there, use edit_plugin_file instead.",
                input_schema: input_of_a_write(),
            },
            ToolDefinition {
                name: EDIT_PLUGIN_FILE,
                description: "Changes one passage of a file that is already there, leaving the rest as it is: a colour in a \
                    drawing, one line of code, one control in ui.json. Quicker and safer than writing the whole file again. \
                    Give the passage exactly as it stands in the file. It has to be there only once, unless you set \
                    `everywhere` to change every place it is found. The same files can be changed as can be written.",
                input_schema: input_of_an_edit(),
            },
            ToolDefinition {
                name: SEARCH_FILES,
                description: "Finds every line that holds some words, in the plugin's own files or in the reference library, \
                    whatever their capitals. Use it to find where something is set without reading every file, and to see how \
                    Akai's example plugins did something. The answer names each file and line.",
                input_schema: input_of_a_search(),
            },
            ToolDefinition {
                name: STYLE_SCREEN,
                description: HOW_TO_STYLE_THE_SCREEN,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "changes": {
                            "type": "array",
                            "description": "The changes to make, in order. Each is an object with a \"change\" and a \"page\". An empty list changes nothing and shows where everything sits.",
                            "items": { "type": "object" }
                        }
                    },
                    "required": ["changes"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: LOOK_AT_SCREEN,
                description: "Shows you the screen as it stands right now, in a few seconds, without building or testing \
                    anything: your pictures are drawn afresh and each page is laid out as the MPC lays it out. Use it freely \
                    while you design: after writing ui.json, after each round of style_screen, after changing a drawing. \
                    Leave `page` out to see every page, or name one. Give `area` as well to see that part of the page at its \
                    real size and enlarged, which is how to judge small lettering, edges, spacing and the detail of a drawing: \
                    the whole page is shown smaller than it is, so small things cannot be judged from it. A page is 1280 by 628. \
                    This does not replace check_plugin, which is still the only way to build and test.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "string", "description": "A page name from ui.json. Leave out to see every page." },
                        "area": {
                            "type": "object",
                            "description": "The part of the page to see enlarged, in the page's own pixels from its top left corner",
                            "properties": { "x": { "type": "integer" }, "y": { "type": "integer" }, "width": { "type": "integer" }, "height": { "type": "integer" } },
                            "required": ["x", "y", "width", "height"]
                        }
                    },
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: LIST_FONTS,
                description: "Lists the fonts on this computer that words drawn as artwork can use (a \"words\" layer added with \
                    style_screen). Such words become part of a picture, so the MPC needs no font for them. Look here before \
                    choosing a font for a title, so you pick one with character that is really there.",
                input_schema: no_input(),
            },
            ToolDefinition {
                name: CHECK_PLUGIN,
                description: "Runs the safety check on the plugin: scans the code, draws its pictures, builds it for \
                    both MPC processors, and plays it through the simulator under several conditions. Call this after every round of \
                    changes, and keep fixing and re-checking until the verdict says it is safe to send. This is the \
                    only way to build or test. Once the plugin builds, the result also shows you each page of its \
                    screen as the MPC lays it out, and each picture you drew, so you can look and correct them.",
                input_schema: no_input(),
            },
        ]
    }

    fn run(&mut self, tool_name: &str, input: &Value) -> ToolOutput {
        let path = input["path"].as_str().unwrap_or_default();
        match tool_name {
            CHECK_PLUGIN => self.check_plugin(),
            LOOK_AT_SCREEN => self.look_at_screen(input),
            READ_PLUGIN_FILE if is_attachment(Path::new(path)) => self.files.show_attachment(path),
            _ => in_words(self.attempt(tool_name, input)),
        }
    }
}

/// The styling tool's description. It has to say everything the AI needs, because the AI sees nothing else of the editor.
const HOW_TO_STYLE_THE_SCREEN: &str = "Styles the plugin's screen the way the musician does in MP Forge's editor: moves and renames \
    things, lays out the inside of a control, hides a picture, and puts colour blocks and words behind the controls. This is the only way \
    to change look.json. Every change is made to the look as it stands, so whatever else was styled by hand is left alone. If one change \
    cannot be made, or the result would not fit the screen, nothing is kept and you are told why. The answer lists where everything sits. \
    A page is 1280 by 628 pixels, measured from its top left corner. A control's slot is 278 by 107. \
    Changes, each an object with \"change\" and \"page\" (a page name from ui.json): \
    (1) {\"change\":\"style\",\"page\":..,\"kind\":\"control\"|\"heading\"|\"picture\",\"name\":..} plus any of: \
    \"place\":{\"x\":..,\"y\":..} where its top left corner goes; \"shownName\":\"..\" the words shown for a control or heading (letters, numbers, simple punctuation); \
    \"size\":{\"width\":..,\"height\":..} for one of your pictures, which is drawn again at that size; \"hidden\":true to take one of your pictures off its page; \"opacity\":0 to 100 to make one of your pictures see-through; \
    \"depth\":\"front\"|\"behind\"|\"back\" to put one of your pictures over the controls, behind them but over the artwork, or behind everything; \
    \"parts\" to lay out the inside of a knob, fader, on-off switch or heading. Give a field as null to take it away again. A control is named by its \
    parameter name and a heading by its words, as ui.json has them. \
    \"parts\" is {\"art\":{\"x\",\"y\",\"width\",\"height\"},\"name\":LABEL,\"value\":LABEL}, each measured from the control's own corner, where LABEL is \
    {\"area\":{\"x\",\"y\",\"width\",\"height\"},\"font\":\"Titillium Web\"|\"Roboto\"|\"Nexa\",\"height\":letter height in pixels,\"colour\":\"#rrggbb\",\"align\":\"left\"|\"centre\"|\"right\", \
    optional \"bold\":true and \"hidden\":true}. A heading has only \"name\". A knob's art must stay square and is drawn again sharp at any size; \
    a fader's art may be any size; a switch's art keeps its own size. As generated, a knob with a kit has art 8,8 90x90, name 110,12 156x36 at height 26, \
    value 110,52 156x42 at height 30; a fader has name 15,8 135x34, value 150,8 113x34, art 15,54 248x36. \
    (2) {\"change\":\"put_back\",\"page\":..,\"kind\":..,\"name\":..} forgets everything styled about that one thing. \
    (3) {\"change\":\"add_artwork\",\"page\":..,\"layer\":LAYER,\"behind\":true|false} lays artwork behind the controls, in front of the page's other artwork \
    unless \"behind\" is true. LAYER is a block {\"kind\":\"block\",\"area\":{\"x\",\"y\",\"width\",\"height\"},\"colour\":\"#rrggbb\", optional \"shape\":\"rectangle\"|\"ellipse\", \
    \"corner\":pixels of rounding, \"edge\":{\"colour\",\"width\"}, \"fade\":{\"colour\":\"#rrggbb\",\"from\":{\"x\",\"y\"},\"to\":{\"x\",\"y\"}} with ends in hundredths of the block, \
    \"opacity\":0 to 100} or words {\"kind\":\"words\",\"text\":\"..\",\"font\":a font on this computer such as \"Helvetica\" or \"Arial\",\"size\":letter height,\"colour\":\"#rrggbb\", \
    \"place\":{\"x\",\"y\"}, optional \"bold\":true, \"opacity\"}. Leave out \"id\": the answer tells you the id it was given. \
    (4) {\"change\":\"change_artwork\",\"page\":..,\"id\":..,\"layer\":{fields to change, null to take one away}}. \
    (5) {\"change\":\"remove_artwork\",\"page\":..,\"id\":..}. \
    (6) {\"change\":\"restack_artwork\",\"page\":..,\"id\":..,\"to\":\"front\"|\"back\"}. \
    Artwork always sits behind the controls and behind your own pictures. You cannot bring in a picture file this way: draw pictures with pictures.json. \
    Putting a control on a page or taking one off is still done in ui.json.";

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    /// A plugin folder and a stand-in SDK folder that has just enough in it to be opened.
    struct Bench {
        plugin: TemporaryFolder,
        sdk: TemporaryFolder,
    }

    impl Bench {
        fn set_up() -> Self {
            let bench = Self { plugin: TemporaryFolder::create("tools-plugin"), sdk: TemporaryFolder::create("tools-sdk") };
            fs::write(bench.plugin.0.join("plugin.json"), "{}").unwrap();
            bench.add_to_sdk("tools/build.py", "");
            bench.add_to_sdk("docs/overview.md", "How plugins work");
            bench
        }

        fn add_to_sdk(&self, relative: &str, contents: &str) {
            let path = self.sdk.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        fn gate(&self) -> Gate {
            Gate::new(Sdk::at(&self.sdk.0).unwrap())
        }
    }

    #[test]
    fn the_tool_list_has_no_shell_network_or_device_access() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let names: Vec<&str> = tools.definitions().iter().map(|tool| tool.name).collect();

        assert_eq!(names, TOOL_NAMES);
    }

    #[test]
    fn writes_then_reads_a_source_file_through_the_tools() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let written = tools.run(WRITE_PLUGIN_FILE, &json!({"path": "src/Echo.cpp", "contents": "// echo"}));
        let read_back = tools.run(READ_PLUGIN_FILE, &json!({"path": "src/Echo.cpp"}));

        assert_eq!(written, ToolOutput::success("Wrote src/Echo.cpp (7 bytes)."));
        assert_eq!(read_back, ToolOutput::success("// echo"));
    }

    #[test]
    fn a_write_outside_the_plugin_fails_and_tells_the_model_why() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let output = tools.run(WRITE_PLUGIN_FILE, &json!({"path": "../../autorun.sh", "contents": "rm -rf /"}));

        assert!(output.is_error);
        assert!(output.content.contains("You can write"));
    }

    #[test]
    fn missing_or_mistyped_input_fails_without_touching_anything() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let output = tools.run(WRITE_PLUGIN_FILE, &json!({"path": "src/A.cpp", "contents": 42}));

        assert_eq!(output, ToolOutput::failure("This tool needs `contents` as text."));
        assert!(!bench.plugin.0.join("src").exists());
    }

    #[test]
    fn the_reference_library_is_readable_but_the_sdk_tools_are_not() {
        let bench = Bench::set_up();
        bench.add_to_sdk("tools/mpc_firmware.py", "dangerous");
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        assert_eq!(tools.run(LIST_REFERENCE, &json!({})), ToolOutput::success("mp-forge/pictures.md\nmp-forge/online.md\ndocs/overview.md"));
        assert_eq!(tools.run(READ_REFERENCE, &json!({"path": "docs/overview.md"})), ToolOutput::success("How plugins work"));
        assert!(tools.run(READ_REFERENCE, &json!({"path": "tools/mpc_firmware.py"})).is_error);
    }

    #[test]
    fn a_picture_the_musician_attached_is_read_as_a_picture_and_cannot_be_written() {
        let bench = Bench::set_up();
        let png = crate::attachments::test_support::png_of(8, 8);
        crate::attachments::AttachmentShelf::of_plugin(&bench.plugin.0).add(&png).unwrap();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let listed = tools.run(LIST_PLUGIN_FILES, &json!({}));
        let shown = tools.run(READ_PLUGIN_FILE, &json!({"path": "attachments/picture-1.png"}));
        let written = tools.run(WRITE_PLUGIN_FILE, &json!({"path": "attachments/picture-1.png", "contents": "x"}));

        assert!(listed.content.contains("attachments/picture-1.png"));
        assert_eq!(shown.illustrations, [Illustration { caption: "attachments/picture-1.png, attached by the musician.".to_string(), png }]);
        assert!(written.is_error);
    }

    fn bench_with_a_screen() -> Bench {
        let bench = Bench::set_up();
        fs::write(bench.plugin.0.join("ui.json"), r#"{"pages": [{"name": "MAIN", "controls": ["Time", "Mix"]}]}"#).unwrap();
        bench
    }

    #[test]
    fn styling_the_screen_keeps_the_look_and_says_where_everything_now_sits() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();
        let move_time = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "place": { "x": 400, "y": 300 } });
        let add_block = json!({ "change": "add_artwork", "page": "MAIN", "layer": { "kind": "block", "area": { "x": 0, "y": 0, "width": 1280, "height": 628 }, "colour": "#102030" } });

        let styled = tools.run(STYLE_SCREEN, &json!({ "changes": [move_time, add_block] }));
        let looked = tools.run(STYLE_SCREEN, &json!({ "changes": [] }));

        let kept = look_of(&bench.plugin.0).unwrap();
        assert!(!styled.is_error, "{}", styled.content);
        assert!(styled.content.contains("control `Time`: at 400, 300, 278 by 107"));
        assert!(styled.content.contains("artwork 1: block of #102030 at 0, 0, 1280 by 628"));
        assert_eq!(kept.artwork_of("MAIN").len(), 1);
        assert!(looked.content.starts_with("Page `MAIN`"));
    }

    #[test]
    fn styling_that_cannot_be_done_keeps_nothing_and_says_why() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();
        let move_time = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Time", "place": { "x": 400, "y": 300 } });
        let off_the_edge = json!({ "change": "style", "page": "MAIN", "kind": "control", "name": "Mix", "place": { "x": 1200, "y": 600 } });
        let no_such_control = json!({ "change": "put_back", "page": "MAIN", "kind": "control", "name": "Speed" });

        let hangs_off = tools.run(STYLE_SCREEN, &json!({ "changes": [move_time.clone(), off_the_edge] }));
        let unknown = tools.run(STYLE_SCREEN, &json!({ "changes": [move_time, no_such_control] }));
        let not_a_list = tools.run(STYLE_SCREEN, &json!({ "changes": "move Time" }));

        assert!(hangs_off.is_error && hangs_off.content.contains("hangs off the edge"));
        assert_eq!(unknown, ToolOutput::failure("Change 2: page `MAIN` has no control called `Speed`. It has: Time, Mix."));
        assert!(not_a_list.is_error);
        assert!(!bench.plugin.0.join("look.json").exists());
    }

    #[test]
    fn the_look_file_itself_still_cannot_be_written() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let written = tools.run(WRITE_PLUGIN_FILE, &json!({"path": "look.json", "contents": "{\"pages\": []}"}));

        assert!(written.is_error);
    }

    #[test]
    fn one_passage_of_a_file_is_changed_and_the_rest_left_alone() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();
        tools.run(WRITE_PLUGIN_FILE, &json!({"path": "pictures/glow.svg", "contents": "<svg><circle fill=\"#ff0000\"/><rect fill=\"#222222\"/></svg>"}));

        let changed = tools.run(EDIT_PLUGIN_FILE, &json!({"path": "pictures/glow.svg", "find": "#ff0000", "replace": "#00c2a8"}));

        assert_eq!(changed, ToolOutput::success("Changed pictures/glow.svg in 1 place."));
        assert_eq!(fs::read_to_string(bench.plugin.0.join("pictures/glow.svg")).unwrap(), "<svg><circle fill=\"#00c2a8\"/><rect fill=\"#222222\"/></svg>");
    }

    #[test]
    fn an_edit_cannot_reach_a_file_that_writing_cannot() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let fixed_file = tools.run(EDIT_PLUGIN_FILE, &json!({"path": "plugin.json", "find": "{", "replace": "{ \"name\": \"x\""}));
        let outside = tools.run(EDIT_PLUGIN_FILE, &json!({"path": "../autorun.sh", "find": "a", "replace": "b"}));

        assert!(fixed_file.is_error && outside.is_error);
        assert_eq!(fs::read_to_string(bench.plugin.0.join("plugin.json")).unwrap(), "{}");
    }

    #[test]
    fn words_are_found_in_the_plugin_or_in_the_reference_library() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let in_plugin = tools.run(SEARCH_FILES, &json!({"text": "mix", "in": "plugin"}));
        let in_reference = tools.run(SEARCH_FILES, &json!({"text": "plugins work", "in": "reference"}));
        let nowhere = tools.run(SEARCH_FILES, &json!({"text": "mix", "in": "the computer"}));

        assert!(in_plugin.content.starts_with("ui.json:1: "));
        assert_eq!(in_reference, ToolOutput::success("docs/overview.md:1: How plugins work"));
        assert!(nowhere.is_error);
    }

    #[test]
    fn the_screen_can_be_looked_at_before_anything_is_built() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let looked = tools.run(LOOK_AT_SCREEN, &json!({}));

        assert_eq!(looked.content, A_QUICK_LOOK);
        assert_eq!(looked.illustrations.len(), 1);
        assert!(looked.illustrations[0].caption.starts_with("Page `MAIN`"));
        assert!(!bench.plugin.0.join("build").exists());
    }

    #[test]
    fn part_of_a_page_is_shown_enlarged_at_its_real_size() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let close_up = tools.run(LOOK_AT_SCREEN, &json!({"page": "MAIN", "area": {"x": 23, "y": 45, "width": 300, "height": 150}}));

        assert_eq!(close_up.illustrations[0].caption, "Page `MAIN` from (23, 45), 300 wide and 150 tall, with each pixel drawn 4 times over.");
        let enlarged = resvg::tiny_skia::Pixmap::decode_png(&close_up.illustrations[0].png).unwrap();
        assert_eq!((enlarged.width(), enlarged.height()), (1200, 600));
    }

    #[test]
    fn a_look_at_a_page_that_is_not_there_says_so() {
        let bench = bench_with_a_screen();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        assert_eq!(tools.run(LOOK_AT_SCREEN, &json!({"page": "MORE"})), ToolOutput::failure(NOTHING_TO_LOOK_AT));
        assert_eq!(tools.run(LOOK_AT_SCREEN, &json!({"area": {"x": 0, "y": 0, "width": 10, "height": 10}})), ToolOutput::failure(A_CLOSE_UP_NEEDS_ITS_PAGE));
    }

    #[test]
    fn the_fonts_for_artwork_are_listed() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        let listed = tools.run(LIST_FONTS, &json!({}));

        assert!(!listed.is_error);
        assert!(listed.content.starts_with("Fonts for words drawn as artwork"));
    }

    #[test]
    fn an_unknown_tool_name_is_an_error() {
        let bench = Bench::set_up();
        let gate = bench.gate();
        let mut tools = PluginTools::open(&bench.plugin.0, &bench.sdk.0, &gate).unwrap();

        assert!(tools.run("run_shell", &json!({"command": "ls"})).is_error);
    }
}
