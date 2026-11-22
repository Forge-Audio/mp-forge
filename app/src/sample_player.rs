//! The player the app keeps for the Sample app on screen: one at a time.
//!
//! The player is this same program started again with `play-sample-app`, so the Sample app's code runs in a
//! process of its own. What the musician touches is written to it a line at a time. What it shows is read as it
//! arrives and the newest kept, for the screen to fetch when it is ready to draw.
//!
//! While an app is out of reach, because it is being written, built or checked, the blacksmith is shown in its
//! place the same way: by this program started again, showing him where the app would show itself.

use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use serde_json::Value;

use workbench_engine::process::command_for;
use workbench_engine::sample_player::{BYTES_SHOWN, PLAY_SAMPLE_APP_COMMAND};
use workbench_engine::sample_smith::{KEEPS_STILL, SHOW_SMITH_COMMAND};

use crate::player::{why_not_taken, writer_to};

const NOT_RUNNING: &str = "The app is not running.";
const HAS_STOPPED: &str = "The app has stopped. Open it again to start it.";

/// The newest thing the app showed that the screen has not fetched yet, and whether the player has ended.
#[derive(Default)]
struct Newest {
    shown: Mutex<Option<Vec<u8>>>,
    has_ended: AtomicBool,
}

struct Running {
    name: String,
    player: Child,
    touches: SyncSender<String>,
    newest: Arc<Newest>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.player.kill();
        let _ = self.player.wait();
    }
}

#[derive(Default)]
pub struct SamplePlayer {
    running: Mutex<Option<Running>>,
}

impl SamplePlayer {
    /// Starts a player for the Sample app built in `app_dir`, in place of whatever was showing.
    pub fn start(&self, name: &str, app_dir: &Path) -> Result<(), String> {
        self.show_with(name, helper_started_with(PLAY_SAMPLE_APP_COMMAND).arg(app_dir))
    }

    /// Shows the blacksmith where the app called `name` would show itself, in place of whatever was showing.
    pub fn show_the_smith(&self, name: &str, keeps_still: bool) -> Result<(), String> {
        let mut smith = helper_started_with(SHOW_SMITH_COMMAND);
        self.show_with(name, if keeps_still { smith.arg(KEEPS_STILL) } else { &mut smith })
    }

    fn show_with(&self, name: &str, helper: &mut Command) -> Result<(), String> {
        let mut player = helper.spawn().map_err(|error| format!("The app could not be started: {error}"))?;
        let its_input = player.stdin.take().ok_or("The app could not be reached.")?;
        let what_it_shows = player.stdout.take().ok_or("The app could not be seen.")?;
        let newest = Arc::new(Newest::default());
        keep_the_newest(what_it_shows, Arc::clone(&newest));
        *self.held() = Some(Running { name: name.to_string(), player, touches: writer_to(its_input), newest });
        Ok(())
    }

    /// Hands the app's player one thing the musician did, without ever waiting for it.
    pub fn tell(&self, name: &str, touched: &Value) -> Result<(), String> {
        let held = self.held();
        let running = held.as_ref().filter(|running| running.name == name).ok_or(NOT_RUNNING)?;
        running.touches.try_send(touched.to_string()).map_err(|refused| why_not_taken(&refused).to_string())
    }

    /// What the app has shown since the screen last asked, or nothing when that has not changed.
    pub fn newly_shown(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let held = self.held();
        let running = held.as_ref().filter(|running| running.name == name).ok_or(NOT_RUNNING)?;
        let shown = running.newest.shown.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
        if shown.is_none() && running.newest.has_ended.load(Ordering::Relaxed) {
            return Err(HAS_STOPPED.to_string());
        }
        Ok(shown)
    }

    pub fn stop(&self) {
        self.held().take();
    }

    fn held(&self) -> MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// This same program, started again to do what `command` names, written to and read from here.
fn helper_started_with(command: &str) -> Command {
    let this_program = env::current_exe().unwrap_or_else(|_| "workbench-app".into());
    let mut helper = command_for(this_program);
    helper.arg(command).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    helper
}

/// Reads what the player shows, one whole showing at a time, and keeps only the newest. A screen that is slow
/// to fetch then skips ahead and never falls behind.
fn keep_the_newest(mut what_it_shows: ChildStdout, newest: Arc<Newest>) {
    thread::spawn(move || {
        let mut shown = vec![0u8; BYTES_SHOWN];
        while what_it_shows.read_exact(&mut shown).is_ok() {
            *newest.shown.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(shown.clone());
        }
        newest.has_ended.store(true, Ordering::Relaxed);
    });
}
