//! The desktop app: one window showing the workbench screen, driving the engine.
//!
//! The same program doubles as the tool server that Claude Code starts, so there is only one thing
//! to install. Started with `serve-tools`, it never opens a window. Started with `play`, it is the live
//! player for one built plugin, and with `read-clock` it watches one for the safety check, so a plugin's
//! code never runs inside the window's own process.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod chatgpt;
mod commands;
mod key_store;
mod payloads;
mod player;
mod upload;
mod workbench;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use tauri::Manager;
use workbench_engine::live_player::{self, PLAY_COMMAND};
use workbench_engine::maker::SERVE_TOOLS_COMMAND;
use workbench_engine::picture_clock::{self, READ_CLOCK_COMMAND};
use workbench_engine::plugin_tools;
use workbench_engine::sdk::Sdk;

use player::LivePlayer;
use workbench::Workbench;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [command, plugin_dir, _sdk_flag, sdk_dir] if command == SERVE_TOOLS_COMMAND => serve_tools(plugin_dir, sdk_dir),
        [command, plugin_library] if command == PLAY_COMMAND => play(plugin_library),
        [command, plugin_library, control] if command == READ_CLOCK_COMMAND => read_clock(plugin_library, control),
        _ => open_window(),
    }
}

/// Standard output carries only the player's answers here, so nothing else may print to it.
fn play(plugin_library: &str) -> ExitCode {
    match live_player::play_on_standard_io(Path::new(plugin_library)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Runs a built plugin in silence and prints how one of its controls turned by itself, for the safety check.
fn read_clock(plugin_library: &str, control: &str) -> ExitCode {
    match picture_clock::print_watched(Path::new(plugin_library), control) {
        Ok(()) => ExitCode::SUCCESS,
        Err(explanation) => {
            eprintln!("{explanation}");
            ExitCode::FAILURE
        }
    }
}

/// Standard output carries only protocol messages here, so nothing else may print to it.
fn serve_tools(plugin_dir: &str, sdk_dir: &str) -> ExitCode {
    let served = Sdk::at(sdk_dir).map_err(|not_found| not_found.to_string()).and_then(|sdk| {
        plugin_tools::serve_on_standard_io(&sdk, Path::new(plugin_dir)).map_err(|error| error.to_string())
    });
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(explanation) => {
            eprintln!("{explanation}");
            ExitCode::FAILURE
        }
    }
}

fn open_window() -> ExitCode {
    let started = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_dir = app.path().app_config_dir()?;
            app.manage(Workbench::at_home().remembering_places_in(&settings_dir));
            Ok(())
        })
        .manage(LivePlayer::default())
        .invoke_handler(tauri::generate_handler![
            commands::library,
            commands::open_plugin,
            commands::add_sample,
            commands::remove_sample,
            commands::start_plugin,
            commands::change_plugin,
            commands::architect_questions,
            commands::roll_the_dice,
            commands::lucky_find,
            commands::open_mpc_catalog,
            commands::remix_readme,
            commands::remix_screenshot,
            commands::check_plugin,
            commands::preview_plugin,
            commands::picture_image,
            commands::skin_image,
            commands::add_attachment,
            commands::stop_work,
            commands::conversation,
            commands::keep_conversation,
            commands::versions,
            commands::go_to_version,
            commands::forget_version,
            commands::fold_old_versions,
            commands::send_to_mpc,
            commands::clipboard_words,
            accounts::ai_accounts,
            accounts::keep_key,
            accounts::forget_key,
            accounts::mpc_password_is_kept,
            accounts::models_offered,
            accounts::sign_in_with_browser,
            accounts::stop_signing_in,
            accounts::open_chatgpt_usage,
            commands::rename_plugin,
            commands::plugins_folder,
            commands::show_plugins_folder,
            commands::choose_plugins_folder,
            commands::choose_test_sound,
            commands::keep_plugins_in,
            commands::delete_plugin,
            commands::begin_editing,
            commands::keep_look,
            commands::add_artwork,
            commands::art_size,
            commands::layer_image,
            commands::page_art_image,
            commands::font_names,
            commands::attachment_image,
            commands::player_start,
            commands::player_tell,
            commands::what_is_needed,
            commands::fetch_build_tools,
            commands::player_stop,
        ])
        .run(tauri::generate_context!());
    match started {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("The app could not start: {error}");
            ExitCode::FAILURE
        }
    }
}
