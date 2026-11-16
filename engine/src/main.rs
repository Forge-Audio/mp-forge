//! Command-line entry to the engine.
//!
//! `workbench gate` runs the safety check on a plugin folder.
//! `workbench make` has Claude build a plugin from a description, then runs the safety check on it.
//! `workbench serve-tools` is started by Claude Code, never by a person: it serves the six plugin tools.
//! `workbench play` is started by the app, never by a person: it plays one built plugin live.
//!
//! Exit code 0: safe to send. 1: not safe to send. 2: could not run at all.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use workbench_engine::agent::Event;
use workbench_engine::ai_settings::AiSettings;
use workbench_engine::claude::Claude;
use workbench_engine::gate::Gate;
use workbench_engine::live_player::{self, PLAY_COMMAND};
use workbench_engine::maker::{Connection, Delivery, MakeError, Maker, PluginRequest, SERVE_TOOLS_COMMAND};
use workbench_engine::picture_clock::{self, READ_CLOCK_COMMAND};
use workbench_engine::plugin_tools;
use workbench_engine::sdk::{PluginIdentity, PluginKind, Sdk};

const USAGE: &str = "\
usage:
  workbench gate <plugin folder> [--sdk <folder>]
  workbench make --name <plugin name> --maker <your name> --kind effect|instrument
                 [--with api|claude-code] [--model fable|opus|sonnet|haiku]
                 [--effort low|medium|high|xhigh|max] [--web on|off]
                 [--plugins <folder>] [--sdk <folder>]
                 <what you want, in your own words>

  --with api          Talk to Claude directly. Needs an `ant auth login` sign-in or ANTHROPIC_API_KEY. (default)
  --with claude-code  Use the Claude Code you already have, signed in however you signed it in.";
const DEFAULT_SDK_DIR: &str = "vendor/mpc-plugin-builder-sdk";
const DEFAULT_PLUGINS_DIR: &str = "plugins";
const SDK_DIR_VARIABLE: &str = "MPC_SDK_DIR";
const NOT_SAFE_TO_SEND: u8 = 1;
const COULD_NOT_RUN: u8 = 2;

/// The command line, split into `--option value` pairs and everything else.
struct Arguments {
    words: Vec<String>,
    options: HashMap<String, String>,
}

impl Arguments {
    fn parse(mut raw: impl Iterator<Item = String>) -> Option<Self> {
        let mut arguments = Self { words: Vec::new(), options: HashMap::new() };
        while let Some(argument) = raw.next() {
            match argument.strip_prefix("--") {
                Some(option) => arguments.options.insert(option.to_string(), raw.next()?),
                None => {
                    arguments.words.push(argument);
                    None
                }
            };
        }
        Some(arguments)
    }

    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    fn sdk_dir(&self) -> PathBuf {
        let from_environment = || env::var_os(SDK_DIR_VARIABLE).map(PathBuf::from);
        self.option("sdk").map(PathBuf::from).or_else(from_environment).unwrap_or_else(|| PathBuf::from(DEFAULT_SDK_DIR))
    }
}

fn main() -> ExitCode {
    let outcome = Arguments::parse(env::args().skip(1)).ok_or_else(|| USAGE.to_string()).and_then(run);
    outcome.unwrap_or_else(|explanation| {
        eprintln!("{explanation}");
        ExitCode::from(COULD_NOT_RUN)
    })
}

fn run(arguments: Arguments) -> Result<ExitCode, String> {
    if let [command, plugin_library] = arguments.words.as_slice()
        && command == PLAY_COMMAND
    {
        return play(plugin_library);
    }
    if let [command, plugin_library, control] = arguments.words.as_slice()
        && command == READ_CLOCK_COMMAND
    {
        return picture_clock::print_watched(Path::new(plugin_library), control).map(|()| ExitCode::SUCCESS);
    }
    let sdk = Sdk::at(arguments.sdk_dir()).map_err(|not_found| not_found.to_string())?;
    match arguments.words.split_first() {
        Some((command, [plugin_dir])) if command == "gate" => Ok(run_gate(sdk, plugin_dir)),
        Some((command, [plugin_dir])) if command == SERVE_TOOLS_COMMAND => serve_tools(sdk, plugin_dir),
        Some((command, [description])) if command == "make" => run_make(sdk, &arguments, description),
        _ => Err(USAGE.to_string()),
    }
}

fn run_gate(sdk: Sdk, plugin_dir: &str) -> ExitCode {
    let report = Gate::new(sdk).inspect(Path::new(plugin_dir));
    println!("{report}");
    exit_code_for(report.may_be_sent())
}

/// Standard output carries only protocol messages here, so nothing else may print to it.
fn serve_tools(sdk: Sdk, plugin_dir: &str) -> Result<ExitCode, String> {
    plugin_tools::serve_on_standard_io(&sdk, Path::new(plugin_dir)).map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// Standard output carries only the player's answers here, so nothing else may print to it.
fn play(plugin_library: &str) -> Result<ExitCode, String> {
    live_player::play_on_standard_io(Path::new(plugin_library)).map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

fn run_make(sdk: Sdk, arguments: &Arguments, description: &str) -> Result<ExitCode, String> {
    let request = PluginRequest { identity: identity_from(arguments)?, description: description.to_string() };
    let maker = Maker::new(sdk, arguments.option("plugins").unwrap_or(DEFAULT_PLUGINS_DIR));
    let settings = AiSettings::from_words(arguments.option("model"), arguments.option("effort")).map_err(|unknown| unknown.to_string())?;
    let settings = settings.with_web_access(arguments.option("web") != Some("off"));
    let delivery = match arguments.option("with").unwrap_or("api") {
        "api" => make_with_api(&maker, &request, settings),
        "claude-code" => make_with_claude_code(&maker, &request, settings),
        _ => return Err(USAGE.to_string()),
    }?;
    print_delivery(&delivery);
    Ok(exit_code_for(delivery.is_ready()))
}

fn make_with_api(maker: &Maker, request: &PluginRequest, settings: AiSettings) -> Result<Delivery, String> {
    let mut claude = Claude::from_environment(settings).map_err(|error| error.to_string())?;
    maker.make(request, Connection::Model(&mut claude), narrate).map_err(explain)
}

fn make_with_claude_code(maker: &Maker, request: &PluginRequest, settings: AiSettings) -> Result<Delivery, String> {
    let this_program = env::current_exe().map_err(|error| error.to_string())?;
    maker.make(request, Connection::ClaudeCode { tool_server_program: &this_program, settings }, narrate).map_err(explain)
}

fn explain(error: MakeError) -> String {
    error.to_string()
}

fn identity_from(arguments: &Arguments) -> Result<PluginIdentity, String> {
    let required = |name: &str| arguments.option(name).map(str::to_string).ok_or_else(|| USAGE.to_string());
    let kind = PluginKind::from_word(&required("kind")?).ok_or_else(|| USAGE.to_string())?;
    Ok(PluginIdentity { name: required("name")?, maker: required("maker")?, kind })
}

fn narrate(event: Event) {
    match event {
        Event::ModelSaid(text) => println!("\nClaude: {text}\n"),
        Event::ToolStarted { name, input } => println!("  {}", plugin_tools::describe_step(name, input)),
        Event::ToolFinished { succeeded: false, .. } => println!("    (that did not work; Claude has been told why)"),
        Event::ToolFinished { .. } | Event::ContextUsed { .. } => {}
        Event::ContextCompacted => println!("    (older material was dropped from the conversation to make room)"),
    }
}

fn print_delivery(delivery: &Delivery) {
    println!("\n{}\n", delivery.report);
    println!("Plugin folder: {}", delivery.plugin_dir.display());
    if !delivery.is_ready() {
        println!("This plugin is NOT ready. Do not copy it to an MPC.");
    }
}

fn exit_code_for(safe_to_send: bool) -> ExitCode {
    if safe_to_send { ExitCode::SUCCESS } else { ExitCode::from(NOT_SAFE_TO_SEND) }
}
