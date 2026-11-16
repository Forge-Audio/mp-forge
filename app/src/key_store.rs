//! Where the musician's keys and passwords are kept: this computer's own store for secrets, and nowhere else.
//! That is the keychain on a Mac, the Credential Manager on Windows and the desktop's Secret Service on Linux.
//!
//! A key is never written to a file, never shown again once it has been given, and never handed to the AI.
//! Each is kept under the name of what it opens: an AI service's word, or the MPC.

/// The name this computer's store files MP Forge's secrets under: the app's own identifier.
const SERVICE: &str = "io.github.forge-audio.mp-forge";
const LONGEST_SECRET: usize = 1024;
const LONGEST_RECORD: usize = 64 * 1024;
const NOT_A_KEY: &str = "That does not look like a key. A key is one unbroken run of letters, digits and marks, with no spaces.";

/// Keeps a secret under an account's name, in place of the one kept there before.
pub fn keep(account: &str, secret: &str) -> Result<(), String> {
    let secret = secret.trim();
    if !could_be_a_secret(secret) {
        return Err(NOT_A_KEY.to_string());
    }
    keychain::set(account, secret)
}

/// Keeps what an account's own sign-in handed over, which is longer than a key and written by the app itself.
pub fn keep_record(account: &str, record: &str) -> Result<(), String> {
    if record.is_empty() || record.len() > LONGEST_RECORD {
        return Err("That is not something the keychain can keep.".to_string());
    }
    keychain::set(account, record)
}

pub fn forget(account: &str) -> Result<(), String> {
    keychain::remove(account)
}

/// The secret itself, for the one moment it is needed. The computer may ask the musician to allow this.
pub fn kept(account: &str) -> Option<String> {
    keychain::get(account)
}

/// Whether a secret is kept, told without reading it, so the computer never has to ask the musician.
pub fn holds(account: &str) -> bool {
    keychain::has(account)
}

fn could_be_a_secret(secret: &str) -> bool {
    let is_one_run = !secret.chars().any(|character| character.is_whitespace() || character.is_control());
    !secret.is_empty() && secret.len() <= LONGEST_SECRET && is_one_run
}

#[cfg(target_os = "macos")]
mod macos_keychain {
    use security_framework::item::{ItemClass, ItemSearchOptions};
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

    use super::SERVICE;

    pub fn set(account: &str, secret: &str) -> Result<(), String> {
        set_generic_password(SERVICE, account, secret.as_bytes()).map_err(|refused| format!("The keychain would not keep it: {refused}"))
    }

    pub fn get(account: &str) -> Option<String> {
        get_generic_password(SERVICE, account).ok().and_then(|secret| String::from_utf8(secret).ok())
    }

    /// Taking away something that is not there is no failure.
    pub fn remove(account: &str) -> Result<(), String> {
        if !has(account) {
            return Ok(());
        }
        delete_generic_password(SERVICE, account).map_err(|refused| format!("The keychain would not let go of it: {refused}"))
    }

    pub fn has(account: &str) -> bool {
        let mut search = ItemSearchOptions::new();
        search.class(ItemClass::generic_password()).service(SERVICE).account(account).load_attributes(true).limit(1);
        search.search().is_ok_and(|found| !found.is_empty())
    }
}

/// The store Windows and Linux each have for secrets, reached through the `keyring` library: the Credential
/// Manager on Windows, the desktop's Secret Service on Linux. The Mac's keychain is reached directly, above,
/// because there a key must never be read just to learn whether it is kept.
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod system_store {
    use keyring::Entry;

    use super::SERVICE;

    /// Windows keeps at most 2,560 bytes under one name, and a character can take four of them. A longer secret
    /// is kept in pieces of this many characters.
    const LONGEST_PIECE: usize = 600;
    /// More pieces than the longest record the app keeps could ever come to.
    const MOST_PIECES: usize = 200;

    pub fn set(account: &str, secret: &str) -> Result<(), String> {
        remove(account)?;
        let pieces = pieces_of(secret);
        pieces.iter().enumerate().try_for_each(|(number, piece)| entry_of(account, number)?.set_password(piece).map_err(would_not_keep))
    }

    pub fn get(account: &str) -> Option<String> {
        let pieces: Vec<String> = (0..MOST_PIECES).map_while(|number| piece_kept(account, number)).collect();
        (!pieces.is_empty()).then(|| pieces.concat())
    }

    /// Taking away something that is not there is no failure.
    pub fn remove(account: &str) -> Result<(), String> {
        let kept = (0..MOST_PIECES).take_while(|number| piece_kept(account, *number).is_some()).count();
        (0..kept).try_for_each(|number| entry_of(account, number)?.delete_credential().map_err(would_not_let_go))
    }

    pub fn has(account: &str) -> bool {
        piece_kept(account, 0).is_some()
    }

    pub(super) fn pieces_of(secret: &str) -> Vec<String> {
        let characters: Vec<char> = secret.chars().collect();
        characters.chunks(LONGEST_PIECE).map(|piece| piece.iter().collect()).collect()
    }

    /// The first piece is kept under the account's own name, so a short secret is one plain entry.
    pub(super) fn name_of_piece(account: &str, number: usize) -> String {
        if number == 0 { account.to_string() } else { format!("{account}#{number}") }
    }

    fn entry_of(account: &str, number: usize) -> Result<Entry, String> {
        Entry::new(SERVICE, &name_of_piece(account, number)).map_err(would_not_keep)
    }

    fn piece_kept(account: &str, number: usize) -> Option<String> {
        entry_of(account, number).ok()?.get_password().ok()
    }

    fn would_not_keep(refused: keyring::Error) -> String {
        format!("This computer's password store would not keep it: {refused}")
    }

    fn would_not_let_go(refused: keyring::Error) -> String {
        format!("This computer's password store would not let go of it: {refused}")
    }
}

#[cfg(target_os = "macos")]
use macos_keychain as keychain;
#[cfg(not(target_os = "macos"))]
use system_store as keychain;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_filed_under_the_identifier_the_app_is_installed_as() {
        let app: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();

        assert_eq!(app["identifier"], SERVICE);
    }

    #[test]
    fn only_one_unbroken_run_of_characters_can_be_a_key() {
        assert!(could_be_a_secret("sk-or-v1-abc123_DEF"));
        assert!(!could_be_a_secret(""));
        assert!(!could_be_a_secret("two words"));
        assert!(!could_be_a_secret("line\nbreak"));
        assert!(!could_be_a_secret(&"k".repeat(LONGEST_SECRET + 1)));
    }

    #[test]
    fn something_that_is_not_a_key_is_refused_before_the_keychain_is_asked() {
        assert_eq!(keep("workbench-test", "not a key"), Err(NOT_A_KEY.to_string()));
    }

    #[test]
    fn a_long_secret_is_cut_into_pieces_windows_can_keep_and_a_short_one_is_one_piece() {
        let long = "é".repeat(1_500);

        let pieces = system_store::pieces_of(&long);

        assert_eq!(pieces.iter().map(|piece| piece.chars().count()).collect::<Vec<_>>(), [600, 600, 300]);
        assert_eq!(pieces.concat(), long);
        assert_eq!(system_store::pieces_of("sk-short"), ["sk-short"]);
        assert_eq!((system_store::name_of_piece("openai", 0), system_store::name_of_piece("openai", 2)), ("openai".to_string(), "openai#2".to_string()));
    }

    #[test]
    #[ignore = "writes to, reads from and clears this computer's real store for secrets"]
    fn the_store_windows_and_linux_use_keeps_a_long_secret_whole_and_forgets_all_of_it() {
        let account = "workbench-test-long-record";
        let long = "r".repeat(1_500);

        system_store::set(account, &long).unwrap();
        let (held, read) = (system_store::has(account), system_store::get(account));
        system_store::set(account, "short-now").unwrap();
        let read_after_a_shorter_one = system_store::get(account);
        system_store::remove(account).unwrap();

        assert!(held);
        assert_eq!(read.as_deref(), Some(long.as_str()));
        assert_eq!(read_after_a_shorter_one.as_deref(), Some("short-now"));
        assert!(!system_store::has(account));
        assert_eq!(system_store::remove(account), Ok(()));
    }

    #[test]
    #[ignore = "writes to, reads from and clears this computer's real keychain"]
    fn a_secret_is_kept_found_without_being_read_and_forgotten() {
        let account = "workbench-test-account";

        keep(account, "  made-up-secret-1  ").unwrap();
        let (held, read) = (holds(account), kept(account));
        forget(account).unwrap();

        assert!(held);
        assert_eq!(read.as_deref(), Some("made-up-secret-1"));
        assert!(!holds(account));
        assert_eq!(forget(account), Ok(()));
    }
}
