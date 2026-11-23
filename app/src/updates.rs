//! Updating MP Forge from inside the app, with Tauri's own updater.
//!
//! The app asks GitHub for one file: the `latest.json` of the newest published release. Its address is always the
//! same and the request carries only the updater's own name and version, so nothing in it tells of the musician
//! or of their plugins. A draft release is not a published one, so it is never on offer.
//!
//! A newer version is fetched and installed only when the musician says so, and what was fetched is checked
//! against the key MP Forge's releases are signed with before any of it is used. Installing ends in the app
//! opening again, so it waits until nothing is being built, checked or changed, and nothing starts meanwhile.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::utils::config::BundleType;
use tauri::utils::platform::{bundle_type, current_exe};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Error, Update, Updater, UpdaterExt};

use crate::accounts::open_in_the_browser;
use crate::workbench::Workbench;

/// Where every installer of the newest version is, with what is new in it.
const DOWNLOAD_PAGE: &str = "https://github.com/Forge-Audio/mp-forge/releases/latest";
const UPDATE_PROGRESS: &str = "update-progress";
const LONGEST_WAIT_TO_REACH_GITHUB: Duration = Duration::from_secs(20);
/// A fetch that has said nothing for this long has stalled, however much of the update it has brought.
const LONGEST_SILENCE_WHILE_FETCHING: Duration = Duration::from_secs(60);
/// Where a Mac keeps a program inside an app. A copy built from source runs from no such place.
const INSIDE_A_MAC_APP: &str = ".app/Contents/MacOS";
const BUILT_FROM_SOURCE: &str = "This copy was built from source, so it cannot update itself. Get an installer from the download page, or fetch the newer source and build it again.";
const NOTHING_ON_OFFER: &str = "No newer version is on offer. Check for updates again.";
const GITHUB_NOT_REACHED: &str = "MP Forge could not reach GitHub to ask for the newest version. Check the connection and try again.";

/// How a copy of MP Forge comes by a newer version.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Updating {
    /// Installed from the .dmg or the setup .exe, or run as an AppImage: it replaces itself.
    ByItself,
    /// Installed as a system package: it replaces itself once the computer's administrator has allowed it.
    AsAPackage,
    /// Built from source: no installer was ever made for it to replace.
    FromSource,
}

impl Updating {
    pub fn of_this_copy() -> Self {
        Self::of_a_copy(what_this_copy_was_installed_as())
    }

    fn of_a_copy(installed_as: Option<BundleType>) -> Self {
        match installed_as {
            Some(BundleType::Deb | BundleType::Rpm) => Self::AsAPackage,
            Some(_) => Self::ByItself,
            None => Self::FromSource,
        }
    }

    /// The word the screen knows this by.
    pub fn word(self) -> &'static str {
        match self {
            Self::ByItself => "byItself",
            Self::AsAPackage => "asAPackage",
            Self::FromSource => "fromSource",
        }
    }
}

/// The installer notes what it made in the program itself. On a Mac it notes nothing and every copy passes for
/// an app, so there a copy built from source is known by not being inside one.
fn what_this_copy_was_installed_as() -> Option<BundleType> {
    bundle_type().filter(|_| !cfg!(target_os = "macos") || runs_inside_a_mac_app())
}

fn runs_inside_a_mac_app() -> bool {
    current_exe().is_ok_and(|program| program.to_string_lossy().contains(INSIDE_A_MAC_APP))
}

/// What asking GitHub for the newest version came to.
enum Standing {
    Newer(Box<Update>),
    Newest,
    /// The newest release carries nothing this copy can install: no `latest.json`, or no file for this system.
    NothingForThisCopy,
}

impl Standing {
    fn said(&self) -> Value {
        match self {
            Self::Newer(update) => json!({ "standing": "newer", "version": update.version }),
            Self::Newest => json!({ "standing": "newest" }),
            Self::NothingForThisCopy => json!({ "standing": "nothingForThisCopy" }),
        }
    }

    fn newer_version(self) -> Option<Update> {
        match self {
            Self::Newer(update) => Some(*update),
            Self::Newest | Self::NothingForThisCopy => None,
        }
    }
}

/// The newer version the last look found, kept so that what the musician says yes to is what gets installed.
#[derive(Default)]
pub struct Offer {
    newer_version: Mutex<Option<Update>>,
}

impl Offer {
    fn keep(&self, newer_version: Option<Update>) {
        *self.held() = newer_version;
    }

    fn standing(&self) -> Option<Update> {
        self.held().clone()
    }

    fn held(&self) -> MutexGuard<'_, Option<Update>> {
        self.newer_version.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Asks GitHub which version is the newest, and says whether it is newer than this copy.
#[tauri::command]
pub async fn newest_version(app: AppHandle, offer: State<'_, Offer>) -> Result<Value, String> {
    let standing = standing_of(&app).await?;
    let said = standing.said();
    offer.keep(standing.newer_version());
    Ok(said)
}

async fn standing_of(app: &AppHandle) -> Result<Standing, String> {
    let asked = match updater_of(app) {
        Ok(updater) => updater.check().await,
        Err(problem) => Err(problem),
    };
    match asked {
        Ok(Some(update)) => Ok(Standing::Newer(Box::new(update))),
        Ok(None) => Ok(Standing::Newest),
        Err(Error::ReleaseNotFound | Error::TargetNotFound(_) | Error::TargetsNotFound(_)) => Ok(Standing::NothingForThisCopy),
        Err(Error::Reqwest(_)) => Err(GITHUB_NOT_REACHED.to_string()),
        Err(problem) => Err(format!("MP Forge could not make out GitHub's answer about the newest version: {problem}.")),
    }
}

/// The updater as the app's settings describe it, giving up on a connection that has gone quiet.
fn updater_of(app: &AppHandle) -> tauri_plugin_updater::Result<Updater> {
    app.updater_builder().configure_client(|client| client.connect_timeout(LONGEST_WAIT_TO_REACH_GITHUB).read_timeout(LONGEST_SILENCE_WHILE_FETCHING)).build()
}

/// Fetches the version on offer, installs it and opens MP Forge again. Called only once the musician has said yes.
#[tauri::command]
pub async fn install_update(app: AppHandle, workbench: State<'_, Workbench>, offer: State<'_, Offer>) -> Result<(), String> {
    if Updating::of_this_copy() == Updating::FromSource {
        return Err(BUILT_FROM_SOURCE.to_string());
    }
    let update = offer.standing().ok_or(NOTHING_ON_OFFER)?;
    let _nothing_else_starts = workbench.claim_to_update()?;
    fetch_and_install(&update, &app).await.map_err(|problem| not_installed(&update, &problem))?;
    app.restart()
}

/// On Windows the installer takes over from here and opens the app again itself.
async fn fetch_and_install(update: &Update, app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let mut fetched = Fetched::default();
    let say_how_far = |chunk: usize, whole: Option<u64>| {
        if let Some(hundredths) = fetched.after(chunk, whole) {
            let _ = app.emit(UPDATE_PROGRESS, json!({ "fetched": hundredths }));
        }
    };
    let say_it_is_all_here = || {
        let _ = app.emit(UPDATE_PROGRESS, json!({ "installing": true }));
    };
    update.download_and_install(say_how_far, say_it_is_all_here).await
}

fn not_installed(update: &Update, problem: &Error) -> String {
    format!("Version {} could not be installed: {problem}. You can get it from the download page instead.", update.version)
}

/// How much of the update has been fetched. It arrives in thousands of small pieces, and the screen is told
/// only when another hundredth of it is here.
#[derive(Default)]
struct Fetched {
    bytes: u64,
    hundredths_said: Option<u64>,
}

impl Fetched {
    /// The hundredths now fetched, when that is more than was last said. Nothing while the whole size is unknown.
    fn after(&mut self, chunk: usize, whole: Option<u64>) -> Option<u64> {
        self.bytes += chunk as u64;
        let hundredths = whole.filter(|&whole| whole > 0).map(|whole| (self.bytes.saturating_mul(100) / whole).min(100))?;
        if self.hundredths_said == Some(hundredths) {
            return None;
        }
        self.hundredths_said = Some(hundredths);
        Some(hundredths)
    }
}

/// Opens the page every installer of the newest version can be had from.
#[tauri::command]
pub fn open_download_page() -> Result<(), String> {
    open_in_the_browser(DOWNLOAD_PAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_made_by_an_installer_replaces_itself() {
        for installed_as in [BundleType::App, BundleType::Dmg, BundleType::Nsis, BundleType::Msi, BundleType::AppImage] {
            assert_eq!(Updating::of_a_copy(Some(installed_as)), Updating::ByItself);
        }
    }

    #[test]
    fn a_copy_installed_as_a_package_needs_the_administrator() {
        assert_eq!(Updating::of_a_copy(Some(BundleType::Deb)), Updating::AsAPackage);
        assert_eq!(Updating::of_a_copy(Some(BundleType::Rpm)), Updating::AsAPackage);
    }

    #[test]
    fn a_copy_built_from_source_does_not_update_itself() {
        assert_eq!(Updating::of_a_copy(None), Updating::FromSource);
        assert_eq!(Updating::of_this_copy(), Updating::FromSource, "the tests are never run from an installed copy");
    }

    #[test]
    fn the_screen_is_told_once_for_each_hundredth_fetched() {
        let mut fetched = Fetched::default();

        let said: Vec<u64> = (0..1000).filter_map(|_| fetched.after(1, Some(1000))).collect();

        assert_eq!(said, (0..=100).collect::<Vec<u64>>());
    }

    #[test]
    fn nothing_is_said_of_a_fetch_whose_size_is_not_known() {
        let mut fetched = Fetched::default();

        assert_eq!(fetched.after(4096, None), None);
        assert_eq!(fetched.after(4096, Some(0)), None);
    }
}
