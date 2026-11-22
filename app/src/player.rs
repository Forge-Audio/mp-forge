//! The live player the app keeps for the plugin on screen: one at a time, started when it is first needed.
//!
//! The player is this same program started again with `play`, so the plugin's code runs in a process of
//! its own. What it says is passed to the screen as `player-event`s, and one last event says it has ended.
//!
//! Requests reach the player through a writer of their own. A plugin that hangs stops reading them, and writing
//! to it from the app's main thread would then freeze the whole app, STOP included.

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

use workbench_engine::live_player::PLAY_COMMAND;
use workbench_engine::online::STAYS_OFFLINE;
use workbench_engine::process::command_for;

const PLAYER_EVENT: &str = "player-event";
const NOT_RUNNING: &str = "The plugin is not being played.";
const NOT_ANSWERING: &str = "The plugin has stopped answering. Stop it and play it again.";
/// Far more than a player that is answering ever has waiting. Only one that has hung lets them pile up this far.
const MOST_REQUESTS_WAITING: usize = 512;

struct Running {
    player: Child,
    requests: SyncSender<String>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.player.kill();
        let _ = self.player.wait();
    }
}

/// A built plugin to play, and whether it may bring sound in from the internet while it plays here.
pub struct Played<'a> {
    pub plugin_library: &'a Path,
    pub may_go_online: bool,
}

#[derive(Default)]
pub struct LivePlayer {
    running: Mutex<Option<Running>>,
}

impl LivePlayer {
    /// Starts a player for one built plugin, in place of any that was running.
    /// `run` is the screen's own number for this start. It comes back on every event, so the screen can
    /// tell what this player says from what an earlier one said.
    pub fn start(&self, run: u32, played: &Played, app: AppHandle) -> Result<(), String> {
        let mut player = player_process(played).spawn().map_err(|error| format!("The player could not be started: {error}"))?;
        let its_input = player.stdin.take().ok_or("The player could not be reached.")?;
        let answers = player.stdout.take().ok_or("The player could not be heard.")?;
        pass_on_what_it_says(answers, run, app);
        *self.held() = Some(Running { player, requests: writer_to(its_input) });
        Ok(())
    }

    /// Hands the player one request without ever waiting for it. The answer comes back as an event.
    pub fn tell(&self, request: &Value) -> Result<(), String> {
        let held = self.held();
        let running = held.as_ref().ok_or(NOT_RUNNING)?;
        running.requests.try_send(request.to_string()).map_err(|refused| why_not_taken(&refused).to_string())
    }

    pub fn stop(&self) {
        self.held().take();
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(crate) fn why_not_taken(refused: &mpsc::TrySendError<String>) -> &'static str {
    match refused {
        mpsc::TrySendError::Full(_) => NOT_ANSWERING,
        mpsc::TrySendError::Disconnected(_) => NOT_RUNNING,
    }
}

/// Starts the writer that passes requests on to the player, and returns the way to hand it one. It ends when
/// the player is stopped or no longer takes what is written.
pub(crate) fn writer_to(its_input: ChildStdin) -> SyncSender<String> {
    let (requests, waiting) = mpsc::sync_channel(MOST_REQUESTS_WAITING);
    thread::spawn(move || write_each(waiting, its_input));
    requests
}

fn write_each(waiting: Receiver<String>, mut its_input: impl Write) {
    for request in waiting {
        let written = writeln!(its_input, "{request}").and_then(|()| its_input.flush());
        if written.is_err() {
            return;
        }
    }
}

/// A plugin that may not go online is told so the way a check tells it, and its stream helper then gives silence.
fn player_process(played: &Played) -> Command {
    let this_program = env::current_exe().unwrap_or_else(|_| "workbench-app".into());
    let mut command = command_for(this_program);
    command.arg(PLAY_COMMAND).arg(played.plugin_library).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if !played.may_go_online {
        command.envs([STAYS_OFFLINE]);
    }
    command
}

fn pass_on_what_it_says(answers: ChildStdout, run: u32, app: AppHandle) {
    thread::spawn(move || {
        let said = BufReader::new(answers).lines().map_while(Result::ok).filter_map(|line| serde_json::from_str::<Value>(&line).ok());
        for answer in said {
            let _ = app.emit(PLAYER_EVENT, json!({ "run": run, "said": answer }));
        }
        let _ = app.emit(PLAYER_EVENT, json!({ "run": run, "said": { "ended": true } }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_told_to_stay_offline(player: &Command) -> bool {
        player.get_envs().any(|(name, value)| name == STAYS_OFFLINE.0 && value.is_some_and(|value| value == STAYS_OFFLINE.1))
    }

    #[test]
    fn a_plugin_is_played_with_its_stream_kept_offline_unless_it_may_go_online() {
        let plugin_library = Path::new("built-plugin.so");

        let kept_offline = player_process(&Played { plugin_library, may_go_online: false });
        let let_online = player_process(&Played { plugin_library, may_go_online: true });

        assert!(is_told_to_stay_offline(&kept_offline));
        assert!(!is_told_to_stay_offline(&let_online));
    }
    use std::io;
    use std::sync::{Arc, Condvar};

    /// Stands in for a player that has hung: it never takes what is written to it until it is let go.
    #[derive(Clone, Default)]
    struct HungInput {
        let_go: Arc<(Mutex<bool>, Condvar)>,
    }

    impl HungInput {
        fn let_go(&self) {
            *self.let_go.0.lock().unwrap() = true;
            self.let_go.1.notify_all();
        }
    }

    impl Write for HungInput {
        fn write(&mut self, written: &[u8]) -> io::Result<usize> {
            let (is_let_go, changed) = &*self.let_go;
            let _held = changed.wait_while(is_let_go.lock().unwrap(), |is_let_go| !*is_let_go).unwrap();
            Ok(written.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn requests_are_written_to_the_player_one_to_a_line_in_the_order_given() {
        let (requests, waiting) = mpsc::sync_channel(MOST_REQUESTS_WAITING);
        let mut written = Vec::new();
        requests.try_send(json!({ "volume": 0.5 }).to_string()).unwrap();
        requests.try_send(json!({ "stop": true }).to_string()).unwrap();
        drop(requests);

        write_each(waiting, &mut written);

        assert_eq!(String::from_utf8(written).unwrap(), "{\"volume\":0.5}\n{\"stop\":true}\n");
    }

    #[test]
    fn a_player_that_has_hung_never_makes_a_request_wait_and_is_said_to_have_stopped_answering() {
        let hung = HungInput::default();
        let (requests, waiting) = mpsc::sync_channel(MOST_REQUESTS_WAITING);
        let writer = thread::spawn({
            let hung = hung.clone();
            move || write_each(waiting, hung)
        });

        let refusals: Vec<&str> = (0..MOST_REQUESTS_WAITING + 2).filter_map(|_| requests.try_send("{}".to_string()).err()).map(|refused| why_not_taken(&refused)).collect();

        assert!(!refusals.is_empty() && refusals.iter().all(|reason| *reason == NOT_ANSWERING));
        hung.let_go();
        drop(requests);
        writer.join().unwrap();
    }
}
