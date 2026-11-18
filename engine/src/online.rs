//! Plugins that go online: the streams of audio a plugin may bring in from the internet, and MP Forge's own
//! helper that brings them in.
//!
//! A plugin's own code may never use the network. A plugin that wants sound from the internet, such as a radio
//! stream, lists its address in `online.json` and reads the sound through MP Forge's stream helper, which is
//! the only code in a plugin that goes online. MP Forge writes the helper into the plugin afresh before every
//! build, with the listed addresses built in, so it can reach those addresses and no others, and nothing a
//! plugin or its maker changed in it survives.
//!
//! All of this is experimental, and happens only when the musician has switched online plugins on.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The addresses a plugin may bring sound in from, kept with the plugin.
pub const ONLINE_FILE: &str = "online.json";
/// What the musician is told when a plugin wants to go online and they have not switched that on.
pub const ONLINE_IS_SWITCHED_OFF: &str = "This plugin brings in sound from the internet, which is experimental and switched off. To build it, switch on \"Online plugins\" in MP Forge's settings, then run the check again.";

/// Said to a plugin run on the computer by a check or a build, where nobody is listening: its stream helper
/// then stays off the network.
pub const STAYS_OFFLINE: (&str, &str) = ("MP_FORGE_STAYS_OFFLINE", "1");

const SOURCE_FOLDER: &str = "src";
/// The folder in a plugin's code that holds what the helper is made of. A plugin's own code may not reach into it.
pub const HELPER_FOLDER: &str = "mp-forge-online";
const STREAMS_FILE: &str = "mp-forge-online/streams.inc";
/// MP Forge's own files, by where they go in a plugin's code. Only these may use the network or keep a thread.
const HELPER_FILES: [(&str, &str); 4] = [
    ("mp_forge_stream.h", include_str!("../helpers/online/mp_forge_stream.h")),
    ("mp_forge_stream.cpp", include_str!("../helpers/online/mp_forge_stream.cpp")),
    ("mp-forge-online/small_sockets.h", include_str!("../helpers/online/small_sockets.h")),
    ("mp-forge-online/minimp3.h", include_str!("../helpers/online/minimp3.h")),
];
const MOST_STREAMS: usize = 16;
const LONGEST_NAME: usize = 32;
const LONGEST_HOST: usize = 100;
const LONGEST_PATH: usize = 200;
const PUNCTUATION_IN_NAMES: &str = " -.&'!+";
const PUNCTUATION_IN_PATHS: &str = "/._~-?=&%+";
const PLAIN_START: &str = "http://";
const USUAL_PORT: u16 = 80;
/// Below this, a port belongs to some other service of a computer, such as mail or remote login.
const LOWEST_OTHER_PORT: u16 = 1024;

/// One stream of audio, such as a radio station: the name the plugin knows it by, and the one address it is fetched from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Stream {
    pub fn address(&self) -> String {
        let port = if self.port == USUAL_PORT { String::new() } else { format!(":{}", self.port) };
        format!("{PLAIN_START}{}{port}{}", self.host, self.path)
    }
}

/// The streams the plugin lists, or everything wrong with its list. A plugin without the file lists none.
pub fn streams_of(plugin: &Path) -> Result<Vec<Stream>, Vec<String>> {
    match fs::read_to_string(plugin.join(ONLINE_FILE)) {
        Ok(written) => streams_written_in(&written),
        Err(_) => Ok(Vec::new()),
    }
}

pub fn streams_written_in(online_file: &str) -> Result<Vec<Stream>, Vec<String>> {
    let parsed: Value = serde_json::from_str(online_file).map_err(|error| vec![format!("{ONLINE_FILE} is not valid JSON: {error}")])?;
    let listed = parsed["streams"].as_array().ok_or_else(|| vec![format!("{ONLINE_FILE} needs a \"streams\" list.")])?;
    if listed.len() > MOST_STREAMS {
        return Err(vec![format!("{ONLINE_FILE} lists {} streams. A plugin can have {MOST_STREAMS} at most.", listed.len())]);
    }
    let (streams, problems): (Vec<_>, Vec<_>) = listed.iter().map(stream_from).partition(Result::is_ok);
    let streams: Vec<Stream> = streams.into_iter().flatten().collect();
    let mut problems: Vec<String> = problems.into_iter().filter_map(Result::err).collect();
    problems.extend(named_twice(&streams));
    if problems.is_empty() { Ok(streams) } else { Err(problems) }
}

fn named_twice(streams: &[Stream]) -> Option<String> {
    let twice = streams.iter().enumerate().find(|(position, stream)| streams[..*position].iter().any(|earlier| earlier.name == stream.name));
    twice.map(|(_, stream)| format!("{ONLINE_FILE} names two streams `{}`. Give each its own name.", stream.name))
}

fn stream_from(listed: &Value) -> Result<Stream, String> {
    let name = listed["name"].as_str().filter(|name| is_a_stream_name(name)).ok_or_else(|| {
        format!("Every stream in {ONLINE_FILE} needs a \"name\" of up to {LONGEST_NAME} letters, digits and spaces.")
    })?;
    let address = listed["address"].as_str().ok_or_else(|| format!("Stream `{name}` needs an \"address\"."))?;
    let (host, port, path) = parts_of(address).map_err(|problem| format!("Stream `{name}`: {problem}"))?;
    Ok(Stream { name: name.to_string(), host, port, path })
}

fn is_a_stream_name(name: &str) -> bool {
    let plain = name.chars().all(|letter| letter.is_ascii_alphanumeric() || PUNCTUATION_IN_NAMES.contains(letter));
    !name.trim().is_empty() && name == name.trim() && name.len() <= LONGEST_NAME && plain
}

/// The computer, the port and the path an address names, when it is one the helper may fetch from.
fn parts_of(address: &str) -> Result<(String, u16, String), String> {
    let after_the_start = address.strip_prefix(PLAIN_START).ok_or_else(|| needs_a_plain_address(address))?;
    let (computer, path) = match after_the_start.find('/') {
        Some(at) => after_the_start.split_at(at),
        None => (after_the_start, "/"),
    };
    let (host, port) = match computer.split_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().map_err(|_| "the port after the colon is not a number.".to_string())?),
        None => (computer, USUAL_PORT),
    };
    refuse_unless(is_a_public_name(host), "its address has to name a computer on the internet, such as stream.example.com. A number, or a computer on your own network, will not do.")?;
    refuse_unless(port == USUAL_PORT || port >= LOWEST_OTHER_PORT, "its port has to be 80, or 1024 and above.")?;
    refuse_unless(is_a_plain_path(path), "the part of its address after the computer's name has characters that cannot be sent.")?;
    Ok((host.to_string(), port, path.to_string()))
}

fn refuse_unless(allowed: bool, why_not: &str) -> Result<(), String> {
    if allowed { Ok(()) } else { Err(why_not.to_string()) }
}

fn needs_a_plain_address(address: &str) -> String {
    if address.starts_with("https://") {
        "its address starts with https://. An encrypted stream cannot be read. Many radio stations also offer one that starts with http://.".to_string()
    } else {
        format!("its address has to start with {PLAIN_START}.")
    }
}

/// A name that is looked up on the internet: letters, digits, hyphens and at least one dot, and not a bare number.
fn is_a_public_name(host: &str) -> bool {
    let plain = host.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '.' || letter == '-');
    let pieces: Vec<&str> = host.split('.').collect();
    let well_formed = pieces.len() >= 2 && pieces.iter().all(|piece| !piece.is_empty() && !piece.starts_with('-') && !piece.ends_with('-'));
    let ends_in_a_word = pieces.last().is_some_and(|last| last.chars().all(|letter| letter.is_ascii_lowercase()));
    let is_this_computer = host == "localhost" || host.ends_with(".local") || host.ends_with(".localhost");
    plain && well_formed && ends_in_a_word && !is_this_computer && host.len() <= LONGEST_HOST
}

fn is_a_plain_path(path: &str) -> bool {
    let plain = path.chars().all(|letter| letter.is_ascii_alphanumeric() || PUNCTUATION_IN_PATHS.contains(letter));
    path.starts_with('/') && path.len() <= LONGEST_PATH && plain
}

/// Gets a plugin's code ready for the check that reads it. A plugin that lists streams is given MP Forge's
/// stream helper, written afresh with those addresses in it. A plugin that lists none has no helper. A plugin
/// that lists streams while online plugins are switched off is refused, as is a list that cannot be used.
pub fn make_ready(plugin: &Path, online_is_switched_on: bool) -> Result<(), Vec<String>> {
    let streams = streams_of(plugin)?;
    let could_not_write = |error: io::Error| vec![format!("MP Forge's stream helper could not be written into the plugin: {error}")];
    if streams.is_empty() {
        return take_helper_out_of(plugin).map_err(could_not_write);
    }
    if !online_is_switched_on {
        return Err(vec![ONLINE_IS_SWITCHED_OFF.to_string()]);
    }
    put_helper_into(plugin, &streams).map_err(could_not_write)
}

fn put_helper_into(plugin: &Path, streams: &[Stream]) -> io::Result<()> {
    fs::create_dir_all(plugin.join(SOURCE_FOLDER).join(HELPER_FOLDER))?;
    helper_for(streams).into_iter().try_for_each(|(place, contents)| fs::write(plugin.join(place), contents))
}

/// MP Forge's stream helper for these streams: every file of it, by its place in a plugin's folder. With a
/// plugin's own code this is all of its source, so whoever is handed the source can read what goes online.
pub fn helper_for(streams: &[Stream]) -> Vec<(PathBuf, String)> {
    let in_the_code = |place: &str| PathBuf::from(SOURCE_FOLDER).join(place);
    let own_files = HELPER_FILES.iter().map(|(place, contents)| (in_the_code(place), contents.to_string()));
    own_files.chain([(in_the_code(STREAMS_FILE), streams_as_code(streams))]).collect()
}

fn take_helper_out_of(plugin: &Path) -> io::Result<()> {
    let code = plugin.join(SOURCE_FOLDER);
    for (place, _) in HELPER_FILES {
        remove_if_there(&code.join(place))?;
    }
    match fs::remove_dir_all(code.join(HELPER_FOLDER)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn remove_if_there(file: &Path) -> io::Result<()> {
    match fs::remove_file(file) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The streams as the table the helper is built with. Every name and address has been checked to hold nothing
/// that could end the text it is written in.
fn streams_as_code(streams: &[Stream]) -> String {
    let rows: String = streams.iter().map(|stream| format!("    {{ \"{}\", \"{}\", {}, \"{}\" }},\n", stream.name, stream.host, stream.port, stream.path)).collect();
    format!(
        "// Written by MP Forge from {ONLINE_FILE} before every build. A change made here is lost.\n\
         static const Stream kStreams[] = {{\n{rows}}};\nstatic const int kStreamCount = {};\n",
        streams.len()
    )
}

/// Whether this file of a plugin's code is one of MP Forge's own, exactly as MP Forge writes it. Only such a
/// file is let off the rules for a plugin's code. `in_the_code` is its path inside the plugin's `src` folder.
pub fn is_mp_forges_own(in_the_code: &Path, contents: &[u8]) -> bool {
    HELPER_FILES.iter().any(|(place, own)| in_the_code == Path::new(place) && contents == own.as_bytes())
}

/// Whether a path inside a plugin's `src` folder is kept for MP Forge's stream helper, so nobody else may write there.
pub fn is_kept_for_the_helper(in_the_code: &Path) -> bool {
    in_the_code.starts_with(HELPER_FOLDER) || HELPER_FILES.iter().any(|(place, _)| in_the_code == Path::new(place))
}

/// Where MP Forge's stream helper sits in a plugin's code, for the guide that tells the AI how to use it.
pub fn helper_header() -> PathBuf {
    PathBuf::from(SOURCE_FOLDER).join(HELPER_FILES[0].0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const ONE_STATION: &str = r#"{ "streams": [{ "name": "Jazz", "address": "http://stream.example.com/jazz.mp3" }] }"#;

    fn refusal_of(address: &str) -> String {
        let listed = format!(r#"{{ "streams": [{{ "name": "A", "address": "{address}" }}] }}"#);
        streams_written_in(&listed).unwrap_err().join(" ")
    }

    fn plugin_listing(online_file: Option<&str>) -> TemporaryFolder {
        let plugin = TemporaryFolder::create("online");
        fs::create_dir_all(plugin.0.join("src")).unwrap();
        if let Some(listed) = online_file {
            fs::write(plugin.0.join(ONLINE_FILE), listed).unwrap();
        }
        plugin
    }

    #[test]
    fn a_stream_is_a_name_and_the_one_address_its_stream_comes_from() {
        let with_a_port = r#"{ "streams": [{ "name": "Deep Space", "address": "http://stream.example.com:8000/deep?type=mp3" }, { "name": "Bare", "address": "http://radio.example.org" }] }"#;

        let streams = streams_written_in(with_a_port).unwrap();

        assert_eq!(streams[0], Stream { name: "Deep Space".to_string(), host: "stream.example.com".to_string(), port: 8000, path: "/deep?type=mp3".to_string() });
        assert_eq!((streams[1].port, streams[1].path.as_str()), (80, "/"));
        assert_eq!((streams[0].address().as_str(), streams[1].address().as_str()), ("http://stream.example.com:8000/deep?type=mp3", "http://radio.example.org/"));
    }

    #[test]
    fn a_plugin_without_the_file_lists_no_streams() {
        assert_eq!(streams_of(&plugin_listing(None).0), Ok(Vec::new()));
    }

    #[test]
    fn an_address_that_must_not_be_fetched_from_is_refused_with_the_reason() {
        assert!(refusal_of("https://stream.example.com/jazz").contains("encrypted stream cannot be read"));
        assert!(refusal_of("ftp://stream.example.com/x").contains("has to start with http://"));
        assert!(refusal_of("http://192.168.1.1/admin").contains("on your own network, will not do"));
        assert!(refusal_of("http://localhost/stream").contains("a computer on the internet"));
        assert!(refusal_of("http://router.local/stream").contains("a computer on the internet"));
        assert!(refusal_of("http://nodots/stream").contains("a computer on the internet"));
        assert!(refusal_of("http://stream.example.com:22/x").contains("80, or 1024 and above"));
        assert!(refusal_of("http://stream.example.com/a b").contains("characters that cannot be sent"));
        assert!(refusal_of(r#"http://stream.example.com/a\\\"b"#).contains("characters that cannot be sent"));
        assert!(refusal_of("http://user:secret@stream.example.com/x").contains("port after the colon is not a number"));
    }

    #[test]
    fn a_list_that_is_malformed_too_long_or_names_a_stream_twice_is_refused() {
        let seventeen: Vec<String> = (0..17).map(|number| format!(r#"{{ "name": "S{number}", "address": "http://s{number}.example.com/" }}"#)).collect();
        let twice = r#"{ "streams": [{ "name": "A", "address": "http://a.example.com/" }, { "name": "A", "address": "http://b.example.com/" }] }"#;
        let sly_name = r#"{ "streams": [{ "name": "A\", \"evil.example.com", "address": "http://a.example.com/" }] }"#;

        assert!(streams_written_in("not json").unwrap_err()[0].contains("not valid JSON"));
        assert!(streams_written_in("{}").unwrap_err()[0].contains("needs a \"streams\" list"));
        assert!(streams_written_in(&format!(r#"{{ "streams": [{}] }}"#, seventeen.join(","))).unwrap_err()[0].contains("16 at most"));
        assert!(streams_written_in(twice).unwrap_err()[0].contains("names two streams `A`"));
        assert!(streams_written_in(sly_name).unwrap_err()[0].contains("needs a \"name\""));
    }

    #[test]
    fn a_plugin_that_lists_streams_is_given_the_helper_with_those_streams_in_it() {
        let plugin = plugin_listing(Some(ONE_STATION));

        make_ready(&plugin.0, true).unwrap();

        let streams = fs::read_to_string(plugin.0.join("src/mp-forge-online/streams.inc")).unwrap();
        assert!(streams.contains("    { \"Jazz\", \"stream.example.com\", 80, \"/jazz.mp3\" },\n};\nstatic const int kStreamCount = 1;"));
        for (place, own) in HELPER_FILES {
            assert_eq!(fs::read_to_string(plugin.0.join("src").join(place)).unwrap(), own);
        }
    }

    #[test]
    fn whatever_was_changed_in_the_helper_is_written_over() {
        let plugin = plugin_listing(Some(ONE_STATION));
        make_ready(&plugin.0, true).unwrap();
        fs::write(plugin.0.join("src/mp_forge_stream.cpp"), "// now it reaches anywhere").unwrap();

        make_ready(&plugin.0, true).unwrap();

        assert_eq!(fs::read_to_string(plugin.0.join("src/mp_forge_stream.cpp")).unwrap(), HELPER_FILES[1].1);
    }

    #[test]
    fn with_online_plugins_switched_off_a_plugin_that_lists_streams_is_refused_and_given_no_helper() {
        let plugin = plugin_listing(Some(ONE_STATION));

        assert_eq!(make_ready(&plugin.0, false), Err(vec![ONLINE_IS_SWITCHED_OFF.to_string()]));
        assert!(!plugin.0.join("src/mp_forge_stream.cpp").exists());
    }

    #[test]
    fn a_plugin_that_lists_no_streams_has_any_helper_taken_out() {
        let plugin = plugin_listing(Some(ONE_STATION));
        make_ready(&plugin.0, true).unwrap();
        fs::remove_file(plugin.0.join(ONLINE_FILE)).unwrap();

        make_ready(&plugin.0, false).unwrap();

        assert!(!plugin.0.join("src/mp_forge_stream.h").exists() && !plugin.0.join("src/mp-forge-online").exists());
    }

    #[test]
    fn only_the_helper_exactly_as_mp_forge_writes_it_counts_as_its_own() {
        let own = HELPER_FILES[1].1.as_bytes();

        assert!(is_mp_forges_own(Path::new("mp_forge_stream.cpp"), own));
        assert!(!is_mp_forges_own(Path::new("mp_forge_stream.cpp"), b"// changed"));
        assert!(!is_mp_forges_own(Path::new("MyPlugin.cpp"), own));
        assert!(is_kept_for_the_helper(Path::new("mp-forge-online/anything.h")) && is_kept_for_the_helper(Path::new("mp_forge_stream.h")));
        assert!(!is_kept_for_the_helper(Path::new("MyPlugin.cpp")));
    }
}
