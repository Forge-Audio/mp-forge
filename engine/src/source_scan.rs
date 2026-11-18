//! Looks through a plugin's source code for things no plugin should do:
//! starting programs, using the network, starting threads, writing files, or stopping the MPC app.
//!
//! This is a tripwire, not a sandbox. Determined code can hide from a text scan.
//! It exists to stop the obvious cases before any of the plugin's code is run on the user's computer.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::online;

const STARTS_PROGRAMS: &str = "starts another program";
const USES_NETWORK: &str = "uses the network";
const STARTS_THREADS: &str = "starts its own threads";
const TOUCHES_FILES: &str = "writes, deletes or opens files directly";
const STOPS_THE_APP: &str = "can stop the whole MPC app";
const STALLS_AUDIO: &str = "pauses the MPC's audio";
const CHANGES_THE_APP: &str = "changes how the MPC app itself runs";
const LOADS_CODE: &str = "loads other code while running";
const RAW_INSTRUCTIONS: &str = "uses raw machine instructions";
const LEAVES_THE_PLUGIN: &str = "pulls in code from outside the plugin";
const REACHES_PAST_HEADERS: &str = "declares system functions by hand, which gets around the header rules";
const GLUES_NAMES: &str = "glues a name together out of pieces, which hides what the code calls";
const REACHES_INTO_THE_HELPER: &str = "reaches into MP Forge's stream helper, which only goes online for the addresses in online.json";

/// The two ways C++ writes the operator that glues pieces into one name.
const NAME_GLUE: [&str; 2] = ["##", "%:%:"];
/// The marks that may stand straight before a quote to make raw text, where a quote or a backslash is plain text.
const RAW_TEXT_MARKS: [&str; 5] = ["R", "LR", "uR", "UR", "u8R"];

/// Names that are forbidden wherever they appear, including as a prefix such as `std::filesystem::path`.
const FORBIDDEN_NAMES: &[(&str, &str)] = &[
    ("extern", REACHES_PAST_HEADERS),
    ("mp_forge_net", REACHES_INTO_THE_HELPER),
    ("mp_forge_online", REACHES_INTO_THE_HELPER),
    ("popen", STARTS_PROGRAMS),
    ("fork", STARTS_PROGRAMS),
    ("vfork", STARTS_PROGRAMS),
    ("execl", STARTS_PROGRAMS),
    ("execlp", STARTS_PROGRAMS),
    ("execle", STARTS_PROGRAMS),
    ("execv", STARTS_PROGRAMS),
    ("execvp", STARTS_PROGRAMS),
    ("execve", STARTS_PROGRAMS),
    ("execvpe", STARTS_PROGRAMS),
    ("posix_spawn", STARTS_PROGRAMS),
    ("posix_spawnp", STARTS_PROGRAMS),
    ("getaddrinfo", USES_NETWORK),
    ("gethostbyname", USES_NETWORK),
    ("std::thread", STARTS_THREADS),
    ("std::jthread", STARTS_THREADS),
    ("std::async", STARTS_THREADS),
    ("pthread_create", STARTS_THREADS),
    ("std::ofstream", TOUCHES_FILES),
    ("std::fstream", TOUCHES_FILES),
    ("std::filesystem", TOUCHES_FILES),
    ("ofstream", TOUCHES_FILES),
    ("fstream", TOUCHES_FILES),
    ("dlopen", LOADS_CODE),
    ("dlsym", LOADS_CODE),
    ("mmap", LOADS_CODE),
    ("mprotect", LOADS_CODE),
    ("asm", RAW_INSTRUCTIONS),
    ("__asm", RAW_INSTRUCTIONS),
    ("__asm__", RAW_INSTRUCTIONS),
];

/// Names that are ordinary words, so they are forbidden only when called as a function.
const FORBIDDEN_CALLS: &[(&str, &str)] = &[
    ("system", STARTS_PROGRAMS),
    ("socket", USES_NETWORK),
    ("fopen", TOUCHES_FILES),
    ("freopen", TOUCHES_FILES),
    ("open", TOUCHES_FILES),
    ("creat", TOUCHES_FILES),
    ("fwrite", TOUCHES_FILES),
    ("unlink", TOUCHES_FILES),
    ("rmdir", TOUCHES_FILES),
    ("rename", TOUCHES_FILES),
    ("mkdir", TOUCHES_FILES),
    ("chmod", TOUCHES_FILES),
    ("truncate", TOUCHES_FILES),
    ("exit", STOPS_THE_APP),
    ("_exit", STOPS_THE_APP),
    ("_Exit", STOPS_THE_APP),
    ("quick_exit", STOPS_THE_APP),
    ("abort", STOPS_THE_APP),
    ("terminate", STOPS_THE_APP),
    ("assert", STOPS_THE_APP),
    ("raise", STOPS_THE_APP),
    ("kill", STOPS_THE_APP),
    ("signal", CHANGES_THE_APP),
    ("sigaction", CHANGES_THE_APP),
    ("setenv", CHANGES_THE_APP),
    ("putenv", CHANGES_THE_APP),
    ("chdir", CHANGES_THE_APP),
    ("sleep", STALLS_AUDIO),
    ("usleep", STALLS_AUDIO),
];

const FORBIDDEN_HEADERS: &[(&str, &str)] = &[
    ("unistd.h", STARTS_PROGRAMS),
    ("spawn.h", STARTS_PROGRAMS),
    ("sys/socket.h", USES_NETWORK),
    ("netinet/in.h", USES_NETWORK),
    ("arpa/inet.h", USES_NETWORK),
    ("netdb.h", USES_NETWORK),
    ("curl/curl.h", USES_NETWORK),
    ("thread", STARTS_THREADS),
    ("future", STARTS_THREADS),
    ("pthread.h", STARTS_THREADS),
    ("filesystem", TOUCHES_FILES),
    ("dlfcn.h", LOADS_CODE),
    ("sys/mman.h", LOADS_CODE),
    ("csignal", CHANGES_THE_APP),
    ("signal.h", CHANGES_THE_APP),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub found: String,
    pub reason: &'static str,
}

impl Violation {
    pub fn describe(&self) -> String {
        let file_name = self.file.file_name().unwrap_or(self.file.as_os_str()).to_string_lossy();
        format!("{file_name} line {}: `{}` {}", self.line, self.found, self.reason)
    }
}

/// A violation before it is tied to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    pub found: String,
    pub reason: &'static str,
}

/// Reads every file of the plugin's code but MP Forge's own stream helper, exactly as MP Forge writes it,
/// which is the one part of a plugin that may go online.
pub fn scan_plugin(plugin: &Path) -> io::Result<Vec<Violation>> {
    let code = plugin.join("src");
    let mut violations = Vec::new();
    for file in files_under(&code)? {
        let written = fs::read(&file)?;
        if online::is_mp_forges_own(file.strip_prefix(&code).unwrap_or(&file), &written) {
            continue;
        }
        let text = String::from_utf8_lossy(&written).into_owned();
        let in_this_file = |hit: Hit| Violation { file: file.clone(), line: hit.line, found: hit.found, reason: hit.reason };
        violations.extend(scan_text(&text).into_iter().map(in_this_file));
    }
    Ok(violations)
}

pub fn scan_text(source: &str) -> Vec<Hit> {
    let mut hits = forbidden_includes(&blank_out(source, Blanking::Comments));
    hits.extend(forbidden_names(&blank_out(source, Blanking::CommentsAndText)));
    hits.sort_by_key(|hit| hit.line);
    hits
}

fn files_under(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(files_under(&path)?);
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// Includes
// ---------------------------------------------------------------------------

fn forbidden_includes(source_without_comments: &str) -> Vec<Hit> {
    numbered_lines(source_without_comments)
        .filter_map(|(line, text)| {
            let header = included_header(text)?;
            let reason = reason_against_header(header)?;
            Some(Hit { line, found: format!("#include {header}"), reason })
        })
        .collect()
}

fn included_header(line: &str) -> Option<&str> {
    let target = include_target(line)?;
    let closing = match target.chars().next()? {
        '<' => '>',
        '"' => '"',
        _ => return None,
    };
    let header = &target[1..];
    Some(&header[..header.find(closing)?])
}

/// What follows `#include` on an include line, or `None` for any other line.
fn include_target(line: &str) -> Option<&str> {
    let directive = line.trim_start().strip_prefix('#')?.trim_start();
    Some(directive.strip_prefix("include")?.trim_start())
}

fn reason_against_header(header: &str) -> Option<&'static str> {
    if header.starts_with('/') || header.contains("..") {
        return Some(LEAVES_THE_PLUGIN);
    }
    if header.contains(online::HELPER_FOLDER) {
        return Some(REACHES_INTO_THE_HELPER);
    }
    reason_listed_for(header, FORBIDDEN_HEADERS)
}

// ---------------------------------------------------------------------------
// Names and calls
// ---------------------------------------------------------------------------

/// An identifier in the code, with any `::` qualification, such as `std::thread`.
struct Name<'a> {
    text: &'a str,
    /// Where the name ends within the text it was found in.
    end: usize,
    follows_member_access: bool,
    follows_scope_operator: bool,
    is_called: bool,
}

impl<'a> Name<'a> {
    fn last_part(&self) -> &'a str {
        self.text.rsplit("::").next().unwrap_or(self.text)
    }

    /// `::exit()` and `std::exit()` name the system's function even when the plugin has its own `exit()`.
    fn explicitly_names_the_system(&self) -> bool {
        self.follows_scope_operator || self.text.starts_with("std::")
    }
}

fn forbidden_names(code_only: &str) -> Vec<Hit> {
    let defined_by_plugin = functions_defined_in(code_only);
    let mut hits = Vec::new();
    let lines_of_code = numbered_lines(code_only).filter(|(_, text)| include_target(text).is_none());
    for (line, text) in lines_of_code {
        hits.extend(name_glue_in(text).map(|glue| Hit { line, found: glue.to_string(), reason: GLUES_NAMES }));
        for name in names_in(text) {
            if let Some(reason) = reason_against_name(&name, &defined_by_plugin) {
                hits.push(Hit { line, found: name.text.to_string(), reason });
            }
        }
    }
    hits
}

/// A name glued together while the code is built is never seen whole here, so the glue itself is refused.
fn name_glue_in(line: &str) -> Option<&'static str> {
    NAME_GLUE.into_iter().find(|glue| line.contains(glue))
}

fn reason_against_name(name: &Name, defined_by_plugin: &HashSet<&str>) -> Option<&'static str> {
    if name.follows_member_access {
        return None;
    }
    let forbidden_anywhere = FORBIDDEN_NAMES
        .iter()
        .find(|(forbidden, _)| is_or_is_inside(name.text, forbidden))
        .map(|(_, reason)| *reason);
    forbidden_anywhere.or_else(|| reason_against_call(name, defined_by_plugin))
}

fn reason_against_call(name: &Name, defined_by_plugin: &HashSet<&str>) -> Option<&'static str> {
    if !name.is_called {
        return None;
    }
    let unqualified = name.text.strip_prefix("std::").unwrap_or(name.text);
    if defined_by_plugin.contains(unqualified) && !name.explicitly_names_the_system() {
        return None;
    }
    reason_listed_for(unqualified, FORBIDDEN_CALLS)
}

/// Names of the functions the plugin writes itself, such as a voice's own `kill()`.
/// A call to one of these is the plugin's code, not the system function of the same name.
fn functions_defined_in(code_only: &str) -> HashSet<&str> {
    names_in(code_only)
        .into_iter()
        .filter(|name| name.is_called && is_followed_by_a_body(&code_only[name.end..]))
        .map(|name| name.last_part())
        .collect()
}

fn is_followed_by_a_body(code_after_name: &str) -> bool {
    skip_parameters(code_after_name.trim_start())
        .map(skip_function_qualifiers)
        .is_some_and(|rest| rest.starts_with('{'))
}

/// Given text starting at `(`, returns what follows the matching `)`.
fn skip_parameters(text: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (position, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 1 => return Some(&text[position + 1..]),
            ')' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

fn skip_function_qualifiers(mut text: &str) -> &str {
    const QUALIFIERS: [&str; 4] = ["const", "noexcept", "override", "final"];
    loop {
        text = text.trim_start();
        match QUALIFIERS.iter().find_map(|qualifier| text.strip_prefix(qualifier)) {
            Some(rest) => text = rest,
            None => return text,
        }
    }
}

fn reason_listed_for(wanted: &str, list: &[(&str, &'static str)]) -> Option<&'static str> {
    list.iter().find(|(listed, _)| *listed == wanted).map(|(_, reason)| *reason)
}

/// True for `std::filesystem` itself and for anything inside it, such as `std::filesystem::path`.
fn is_or_is_inside(name: &str, forbidden: &str) -> bool {
    name.strip_prefix(forbidden).is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

fn names_in(line: &str) -> Vec<Name<'_>> {
    let bytes = line.as_bytes();
    let mut names = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position].is_ascii_digit() {
            position = end_of_word(bytes, position);
        } else if starts_identifier(bytes[position]) {
            let end = end_of_qualified_name(bytes, position);
            let code_before = line[..position].trim_end();
            names.push(Name {
                text: &line[position..end],
                end,
                follows_member_access: code_before.ends_with('.') || code_before.ends_with("->"),
                follows_scope_operator: code_before.ends_with("::"),
                is_called: line[end..].trim_start().starts_with('('),
            });
            position = end;
        } else {
            position += 1;
        }
    }
    names
}

fn end_of_qualified_name(bytes: &[u8], start: usize) -> usize {
    let mut end = end_of_word(bytes, start);
    while bytes[end..].starts_with(b"::") && bytes.get(end + 2).copied().is_some_and(starts_identifier) {
        end = end_of_word(bytes, end + 2);
    }
    end
}

fn end_of_word(bytes: &[u8], start: usize) -> usize {
    let length = bytes[start..].iter().take_while(|byte| continues_identifier(**byte)).count();
    start + length
}

fn starts_identifier(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn continues_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn numbered_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().map(|(index, line)| (index + 1, line))
}

// ---------------------------------------------------------------------------
// Blanking out comments and quoted text, so words inside them are not mistaken for code
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Blanking {
    Comments,
    CommentsAndText,
}

#[derive(Clone, Copy)]
enum Reading {
    Code,
    LineComment,
    BlockComment,
    QuotedText { closing_quote: char },
    RawText,
}

/// Replaces comments (and optionally the contents of quoted text) with spaces, keeping every line break.
/// A line of code that ends in a backslash runs on into the next, as it does when the code is built.
fn blank_out(source: &str, blanking: Blanking) -> String {
    let mut cleaner = Cleaner { blanking, reading: Reading::Code, cleaned: String::with_capacity(source.len()), raw_text: RawText::default(), line_breaks_owed: 0 };
    let mut characters = source.chars().peekable();
    while let Some(current) = characters.next() {
        let consumed_next = cleaner.take(current, characters.peek().copied());
        if consumed_next {
            characters.next();
        }
    }
    cleaner.cleaned
}

struct Cleaner {
    blanking: Blanking,
    reading: Reading,
    cleaned: String,
    raw_text: RawText,
    /// Line breaks taken out where lines were joined. They are given back at the end of the joined line, so every
    /// later line keeps its number.
    line_breaks_owed: usize,
}

/// Raw text, `R"mark(...)mark"`, as far as it has been read. It ends only at its own closing mark.
#[derive(Default)]
struct RawText {
    read: String,
    mark_ends_at: Option<usize>,
}

impl RawText {
    fn take(&mut self, character: char) {
        if self.mark_ends_at.is_none() && character == '(' {
            self.mark_ends_at = Some(self.read.len());
        }
        self.read.push(character);
    }

    fn has_closed(&self) -> bool {
        let Some(mark_ends_at) = self.mark_ends_at else { return false };
        let closing = format!("){}\"", &self.read[..mark_ends_at]);
        self.read.len() > mark_ends_at + closing.len() && self.read.ends_with(&closing)
    }
}

impl Cleaner {
    /// Handles one character. Returns true when it also used up the character after it.
    fn take(&mut self, current: char, next: Option<char>) -> bool {
        match self.reading {
            Reading::Code => self.take_code(current, next),
            Reading::LineComment => self.take_line_comment(current),
            Reading::BlockComment => self.take_block_comment(current, next),
            Reading::QuotedText { closing_quote } => self.take_quoted_text(current, next, closing_quote),
            Reading::RawText => self.take_raw_text(current),
        }
    }

    fn take_code(&mut self, current: char, next: Option<char>) -> bool {
        match (current, next) {
            ('/', Some('/')) => self.start_comment(Reading::LineComment),
            ('/', Some('*')) => self.start_comment(Reading::BlockComment),
            ('\\', Some('\n')) => self.join_the_next_line(),
            ('\n', _) => self.end_the_line(),
            ('"', _) if RAW_TEXT_MARKS.contains(&self.word_so_far().as_str()) => self.start_raw_text(),
            ('"', _) => self.start_quoted_text('"'),
            ('\'', _) if !self.inside_a_number() => self.start_quoted_text('\''),
            _ => {
                self.cleaned.push(current);
                false
            }
        }
    }

    fn join_the_next_line(&mut self) -> bool {
        self.line_breaks_owed += 1;
        true
    }

    fn end_the_line(&mut self) -> bool {
        let line_breaks = 1 + std::mem::take(&mut self.line_breaks_owed);
        self.cleaned.extend(std::iter::repeat_n('\n', line_breaks));
        false
    }

    fn start_raw_text(&mut self) -> bool {
        self.reading = Reading::RawText;
        self.raw_text = RawText::default();
        self.cleaned.push('"');
        false
    }

    fn take_raw_text(&mut self, current: char) -> bool {
        self.raw_text.take(current);
        if self.raw_text.has_closed() {
            self.reading = Reading::Code;
            self.cleaned.push(current);
        } else {
            self.push_text(current);
        }
        false
    }

    fn start_comment(&mut self, comment: Reading) -> bool {
        self.reading = comment;
        self.cleaned.push_str("  ");
        true
    }

    fn start_quoted_text(&mut self, quote: char) -> bool {
        self.reading = Reading::QuotedText { closing_quote: quote };
        self.cleaned.push(quote);
        false
    }

    /// C++ allows `1'000'000`; that apostrophe does not start quoted text. One after a word does, as in `L'x'`,
    /// so only a word that begins with a digit counts as a number.
    fn inside_a_number(&self) -> bool {
        self.word_so_far().starts_with(|first: char| first.is_ascii_digit())
    }

    /// The word, or the number with its separators, that the code read so far ends with.
    fn word_so_far(&self) -> String {
        let is_part_of_a_word = |character: &char| character.is_ascii_alphanumeric() || *character == '_' || *character == '\'';
        let backwards: Vec<char> = self.cleaned.chars().rev().take_while(is_part_of_a_word).collect();
        backwards.into_iter().rev().collect()
    }

    fn take_line_comment(&mut self, current: char) -> bool {
        if current == '\n' {
            self.reading = Reading::Code;
        }
        self.push_blank(current);
        false
    }

    fn take_block_comment(&mut self, current: char, next: Option<char>) -> bool {
        if (current, next) == ('*', Some('/')) {
            self.reading = Reading::Code;
            self.cleaned.push_str("  ");
            return true;
        }
        self.push_blank(current);
        false
    }

    fn take_quoted_text(&mut self, current: char, next: Option<char>, closing_quote: char) -> bool {
        if current == '\\' {
            self.push_text(current);
            self.push_text(next.unwrap_or(' '));
            return next.is_some();
        }
        if current == closing_quote || current == '\n' {
            self.reading = Reading::Code;
            self.cleaned.push(current);
            return false;
        }
        self.push_text(current);
        false
    }

    fn push_text(&mut self, character: char) {
        match self.blanking {
            Blanking::Comments => self.cleaned.push(character),
            Blanking::CommentsAndText => self.push_blank(character),
        }
    }

    fn push_blank(&mut self, character: char) {
        self.cleaned.push(if character == '\n' { '\n' } else { ' ' });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reasons(source: &str) -> Vec<&'static str> {
        scan_text(source).into_iter().map(|hit| hit.reason).collect()
    }

    #[test]
    fn ordinary_audio_code_is_clean() {
        let source = r#"
            #include "mpc_plugin.h"
            #include <cmath>
            #include <fstream>
            // Never call system() or exit() in here.
            /* std::thread is not allowed either */
            const char* hint = "do not fork() or open() anything";
            float process(float x) { return std::tanh(x * 1'000.0f); }
            void Voice::kill() { level = 0.0f; }
            void stopAll() { voices[0].kill(); other->exit(); }
            float signal = 0.0f;
            std::ifstream wavetable(path);
        "#;

        assert_eq!(scan_text(source), []);
    }

    #[test]
    fn a_plugins_own_function_may_share_a_name_with_a_system_one() {
        let source = "
            struct Voice {
                void start(int type) { kill(); }
                void kill() noexcept { active = false; }
                bool active = false;
            };
        ";

        assert_eq!(scan_text(source), []);
    }

    #[test]
    fn naming_the_system_function_outright_is_caught_even_then() {
        let source = "void kill() {}\nvoid f() { ::kill(1, 9); }\nvoid g() { std::exit(1); }\nvoid exit() {}";

        assert_eq!(reasons(source), [STOPS_THE_APP, STOPS_THE_APP]);
    }

    #[test]
    fn declaring_a_system_function_by_hand_is_caught() {
        let source = r#"extern "C" int getpid();"#;

        assert_eq!(reasons(source), [REACHES_PAST_HEADERS]);
    }

    #[test]
    fn starting_a_program_is_caught() {
        assert_eq!(reasons(r#"void f() { system("rm -rf /"); }"#), [STARTS_PROGRAMS]);
        assert_eq!(reasons("int f() { return ::fork(); }"), [STARTS_PROGRAMS]);
        assert_eq!(reasons("auto run = popen;"), [STARTS_PROGRAMS]);
    }

    #[test]
    fn stopping_the_app_is_caught_with_or_without_std() {
        assert_eq!(reasons("void f() { exit(1); }"), [STOPS_THE_APP]);
        assert_eq!(reasons("void f() { std::abort(); }"), [STOPS_THE_APP]);
        assert_eq!(reasons("void f(int n) { assert(n > 0); }"), [STOPS_THE_APP]);
    }

    #[test]
    fn threads_network_and_file_writing_are_caught() {
        assert_eq!(reasons("std::thread worker;"), [STARTS_THREADS]);
        assert_eq!(reasons("int s = socket(2, 1, 0);"), [USES_NETWORK]);
        assert_eq!(reasons("std::ofstream out(path);"), [TOUCHES_FILES]);
        assert_eq!(reasons("std::filesystem::remove(path);"), [TOUCHES_FILES]);
    }

    #[test]
    fn forbidden_headers_are_caught_unless_commented_out() {
        assert_eq!(reasons("#include <thread>"), [STARTS_THREADS]);
        assert_eq!(reasons("#  include <sys/socket.h>"), [USES_NETWORK]);
        assert_eq!(reasons("// #include <thread>"), [] as [&str; 0]);
    }

    #[test]
    fn includes_that_reach_outside_the_plugin_are_caught() {
        assert_eq!(reasons(r#"#include "../../other/secret.h""#), [LEAVES_THE_PLUGIN]);
        assert_eq!(reasons("#include </etc/passwd>"), [LEAVES_THE_PLUGIN]);
    }

    #[test]
    fn a_number_with_separators_does_not_hide_what_follows() {
        assert_eq!(reasons(r#"int n = 1'000'000; void f() { system("x"); }"#), [STARTS_PROGRAMS]);
    }

    #[test]
    fn a_marked_character_holding_a_quote_does_not_hide_what_follows() {
        assert_eq!(reasons(r#"wchar_t a = L'"'; void f() { system("x"); } wchar_t b = L'"';"#), [STARTS_PROGRAMS]);
        assert_eq!(reasons(r#"auto a = u8'"'; void f() { fork(); }"#), [STARTS_PROGRAMS]);
    }

    #[test]
    fn raw_text_holding_a_quote_does_not_hide_what_follows() {
        assert_eq!(reasons(r###"auto a = R"(")"; void f() { system("x"); } auto b = R"(")";"###), [STARTS_PROGRAMS]);
        assert_eq!(reasons(r###"auto a = u8R"end(a )" b)end"; void f() { abort(); }"###), [STOPS_THE_APP]);
    }

    #[test]
    fn what_raw_text_says_is_not_mistaken_for_code() {
        assert_eq!(reasons(r###"const char* hint = R"(never call system("x") or exit(1))";"###), [] as [&str; 0]);
    }

    #[test]
    fn a_name_split_over_two_lines_is_read_whole_and_later_lines_keep_their_numbers() {
        let hits = scan_text("void f() { sys\\\ntem(\"x\"); }\nvoid g() { abort(); }\n");

        assert_eq!(hits, [Hit { line: 1, found: "system".to_string(), reason: STARTS_PROGRAMS }, Hit { line: 3, found: "abort".to_string(), reason: STOPS_THE_APP }]);
    }

    #[test]
    fn a_header_named_over_two_lines_is_caught() {
        assert_eq!(reasons("#inc\\\nlude <thread>"), [STARTS_THREADS]);
    }

    #[test]
    fn gluing_a_name_together_out_of_pieces_is_caught() {
        assert_eq!(reasons("#define CALL(a, b) a##b\nvoid f() { CALL(sys, tem)(\"x\"); }"), [GLUES_NAMES]);
        assert_eq!(reasons("#define CALL(a, b) a%:%:b"), [GLUES_NAMES]);
        assert_eq!(reasons("const char* sharp = \"C## minor\"; // a ## in a comment"), [] as [&str; 0]);
    }

    #[test]
    fn the_line_of_each_hit_is_reported() {
        let hits = scan_text("/* a\n   comment */\nfloat x;\nvoid f() { abort(); }\n");

        assert_eq!(hits, [Hit { line: 4, found: "abort".to_string(), reason: STOPS_THE_APP }]);
    }
}
