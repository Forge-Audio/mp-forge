//! Getting a plugin ready to be listed in the Open MPC catalog.
//!
//! The catalog is the community's own list of open plugins for the MPC. It lists a plugin from three things its
//! maker puts on GitHub: a repository with the plugin's source and a licence, a release with the built plugin as
//! a zip, and one small file added to the catalog itself. MP Forge does none of the GitHub part. It makes the
//! three things in a folder the musician chooses, ready to upload, and writes the steps beside them.
//!
//! The repository's README is the plugin's guide, with the pictures of its screen, so the plugin has a proper
//! page of its own.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest::{SHA256, digest};

use crate::guide::{self, Facts, GuideError, Words};
use crate::guide_readme::{AsShared, picture_file_of, readme_of};
use crate::online;
use crate::ordered_json::Json;
use crate::project_file::carried_files_in;
use crate::small_zip::Zip;

const MANIFEST: &str = "plugin.json";
const BUILD_FOLDER: &str = "build";
const README: &str = "README.md";
const LICENSE_FILE: &str = "LICENSE";
const GUIDE_IN_THE_REPOSITORY: &str = "docs/guide.html";
const STEPS_FILE: &str = "START HERE.md";
const FOR_THE_REPOSITORY: &str = "1 - put in your repository";
const FOR_THE_RELEASE: &str = "2 - attach to your release";
const FOR_THE_CATALOG: &str = "3 - add to the catalog";
/// What the catalog knows a built plugin's zip by, when it is the folder Akai's tools build and nothing more.
const ZIP_ENDING: &str = "mpc-sdk.zip";
const THE_CATALOG: &str = "open-mpc/mpc-vst-plugins";
const HOW_OPEN_MPC_SAYS_IT: &str = "https://open-mpc.github.io/workflow.html#list-it-in-the-catalog";
const LONGEST_SUMMARY: usize = 350;
const LONGEST_REPOSITORY: usize = 100;
const SECONDS_IN_A_DAY: u64 = 86_400;
const MIT_TEXT: &str = include_str!("licenses/MIT.txt");
const GPL_3_TEXT: &str = include_str!("../../LICENSE");

/// The licence a plugin is shared under. The catalog lists open plugins only, and these are the two its plugins
/// most often carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum License {
    /// Anyone may do anything with it, as long as they keep the maker's name on it.
    Mit,
    /// Anyone may change and share it, as long as what they share stays open the same way.
    Gpl3,
}

impl License {
    pub fn named(name: &str) -> Option<License> {
        [License::Mit, License::Gpl3].into_iter().find(|license| license.name() == name)
    }

    /// The name licences are known by in lists, which is how the catalog wants it written.
    pub fn name(self) -> &'static str {
        match self {
            License::Mit => "MIT",
            License::Gpl3 => "GPL-3.0-only",
        }
    }

    fn text_for(self, holder: &str, year: i64) -> String {
        match self {
            License::Mit => MIT_TEXT.replace("{{YEAR}}", &year.to_string()).replace("{{HOLDER}}", holder),
            License::Gpl3 => GPL_3_TEXT.to_string(),
        }
    }
}

/// What the musician decides about a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// Their repository on GitHub, as `owner/name`.
    pub repository: String,
    pub license: License,
    /// One line for the plugin's card in the catalog.
    pub summary: String,
}

impl Listing {
    /// The listing as the musician gave it, with a repository's whole address cut down to `owner/name`.
    pub fn given(repository: &str, license: License, summary: &str) -> Listing {
        Listing { repository: repository_named_in(repository), license, summary: summary.trim().to_string() }
    }

    /// Everything about the listing the catalog would send back.
    pub fn problems(&self) -> Vec<String> {
        let checks = [
            (is_a_repository(&self.repository), "The repository has to be written as owner/name, such as yourname/tape-echo."),
            (!self.summary.is_empty(), "The catalog needs one line that says what the plugin sounds like or does."),
            (self.summary.len() <= LONGEST_SUMMARY, "That line is too long for a card in the catalog. Keep it under 350 letters."),
            (self.summary.chars().all(is_plain), "The catalog takes plain letters and punctuation only in that line: no curly quotes, long dashes or accents."),
        ];
        checks.iter().filter(|(holds, _)| !holds).map(|(_, problem)| problem.to_string()).collect()
    }
}

fn repository_named_in(given: &str) -> String {
    let without_the_site = given.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("github.com/");
    without_the_site.trim_end_matches('/').trim_end_matches(".git").to_string()
}

fn is_a_repository(repository: &str) -> bool {
    let is_a_name = |part: &str| !part.is_empty() && part.chars().all(|letter| letter.is_ascii_alphanumeric() || "_.-".contains(letter));
    let parts: Vec<&str> = repository.split('/').collect();
    parts.len() == 2 && parts.iter().all(|part| is_a_name(part)) && repository.len() <= LONGEST_REPOSITORY
}

fn is_plain(letter: char) -> bool {
    letter == ' ' || letter.is_ascii_graphic()
}

/// Why a plugin was not got ready. Nothing is left behind where it was to go.
#[derive(Debug)]
pub enum NotListed {
    NotBuiltYet,
    DoesNotFit(Vec<String>),
    Io(io::Error),
}

impl fmt::Display for NotListed {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NotListed::NotBuiltYet => write!(formatter, "The plugin has not been built yet. Run its safety check first."),
            NotListed::DoesNotFit(problems) => write!(formatter, "{}", problems.join(" ")),
            NotListed::Io(cause) => write!(formatter, "The files could not be written: {cause}."),
        }
    }
}

impl From<io::Error> for NotListed {
    fn from(cause: io::Error) -> Self {
        NotListed::Io(cause)
    }
}

impl From<GuideError> for NotListed {
    fn from(problem: GuideError) -> Self {
        match problem {
            GuideError::Io(cause) => NotListed::Io(cause),
            _ => NotListed::NotBuiltYet,
        }
    }
}

/// What the musician is shown before they decide: how the plugin would be listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsItWouldBeListed {
    pub id: String,
    pub author: String,
    pub version: String,
    /// A line for the card, taken from the plugin's guide when it has one.
    pub suggested_summary: String,
    pub guide_is_written: bool,
}

pub fn as_it_would_be_listed(plugin_dir: &Path) -> Result<AsItWouldBeListed, NotListed> {
    let plugin = BuiltPlugin::in_folder(plugin_dir)?;
    Ok(AsItWouldBeListed {
        id: plugin.id(),
        author: plugin.facts.maker.clone(),
        version: plugin.version.clone(),
        suggested_summary: first_sentence_of(&plugin.words.about),
        guide_is_written: plugin.guide_is_written,
    })
}

fn first_sentence_of(words: &str) -> String {
    let ends_at = words.find(". ").map_or(words.len(), |full_stop| full_stop + 1);
    words[..ends_at].chars().filter(|letter| is_plain(*letter)).take(LONGEST_SUMMARY).collect()
}

/// Makes everything the listing needs, in a new folder inside `into`, and returns that folder. A folder that is
/// already there is never written over: the new one gets a number after its name.
pub fn prepare(plugin_dir: &Path, listing: &Listing, into: &Path) -> Result<PathBuf, NotListed> {
    let mut problems = listing.problems();
    let plugin = BuiltPlugin::in_folder(plugin_dir)?;
    problems.extend(plugin.problems());
    if !problems.is_empty() {
        return Err(NotListed::DoesNotFit(problems));
    }
    let folder = first_free_folder(into, &format!("{} for Open MPC", plugin.facts.name));
    let made = plugin.write_listing(listing, &folder);
    if made.is_err() {
        let _ = fs::remove_dir_all(&folder);
    }
    made.map(|()| folder).map_err(NotListed::Io)
}

fn first_free_folder(into: &Path, name: &str) -> PathBuf {
    let numbered = (2..).map(|number| into.join(format!("{name} {number}")));
    std::iter::once(into.join(name)).chain(numbered).find(|folder| !folder.exists()).expect("there is always another number")
}

/// A plugin that has been built, with what its guide says of it.
struct BuiltPlugin {
    project: PathBuf,
    manifest: Json,
    /// The folder Akai's tools built: what goes on an MPC.
    built: PathBuf,
    version: String,
    facts: Facts,
    words: Words,
    guide_is_written: bool,
}

impl BuiltPlugin {
    fn in_folder(project: &Path) -> Result<Self, NotListed> {
        let manifest = fs::read_to_string(project.join(MANIFEST)).ok().and_then(|written| Json::parse(&written).ok()).ok_or(NotListed::NotBuiltYet)?;
        let facts = guide::facts_of(project)?;
        let built = project.join(BUILD_FOLDER).join(format!("{} - VST - {}", facts.maker, facts.name));
        if !built.join(MANIFEST).is_file() {
            return Err(NotListed::NotBuiltYet);
        }
        let version = manifest.field("version").and_then(Json::as_text).unwrap_or_default().to_string();
        let kept = guide::kept_words(project, &facts);
        Ok(Self { project: project.to_path_buf(), manifest, built, version, guide_is_written: kept.is_some(), words: kept.unwrap_or_default(), facts })
    }

    fn problems(&self) -> Vec<String> {
        let checks = [
            (is_a_version(&self.version), format!("The plugin's version is written as `{}`. The catalog needs three numbers, such as 1.0.0.", self.version)),
            (!self.id().is_empty(), "The plugin's name has no letters or digits the catalog could list it under.".to_string()),
        ];
        checks.into_iter().filter(|(holds, _)| !holds).map(|(_, problem)| problem).collect()
    }

    /// What the catalog knows the plugin by: its name in small letters and digits, with hyphens between.
    fn id(&self) -> String {
        words_joined_by_hyphens(&self.facts.name.to_ascii_lowercase())
    }

    fn zip_name(&self) -> String {
        format!("{}-{}-{ZIP_ENDING}", words_joined_by_hyphens(&self.facts.name), self.version)
    }

    fn tag(&self) -> String {
        format!("v{}", self.version)
    }

    fn write_listing(&self, listing: &Listing, folder: &Path) -> io::Result<()> {
        let zip = self.zipped(listing)?;
        self.write_the_repository(listing, &folder.join(FOR_THE_REPOSITORY))?;
        write_into(&folder.join(FOR_THE_RELEASE).join(self.zip_name()), &zip)?;
        write_into(&folder.join(FOR_THE_CATALOG).join(format!("{}.json", self.id())), self.catalog_entry(listing).as_bytes())?;
        write_into(&folder.join(STEPS_FILE), self.steps(listing, &checksum_of(&zip)).as_bytes())
    }

    // ---------------------------------------------------------------------------------------
    // The repository: the plugin's source, its licence and its guide as the page it opens on
    // ---------------------------------------------------------------------------------------

    fn write_the_repository(&self, listing: &Listing, repository: &Path) -> io::Result<()> {
        for carried in carried_files_in(&self.project) {
            write_into(&repository.join(&carried), &fs::read(self.project.join(&carried))?)?;
        }
        write_into(&repository.join(MANIFEST), self.manifest_as_listed(&self.manifest, listing).as_bytes())?;
        write_into(&repository.join(README), self.readme(listing).as_bytes())?;
        write_into(&repository.join(LICENSE_FILE), listing.license.text_for(&self.facts.maker, this_year()).as_bytes())?;
        for (page_number, page) in self.pages_with_a_picture() {
            write_into(&repository.join(picture_file_of(page_number)), &page.picture)?;
        }
        if self.guide_is_written {
            write_into(&repository.join(GUIDE_IN_THE_REPOSITORY), guide::page_of(&self.facts, &self.words).as_bytes())?;
        }
        self.write_what_goes_online(repository)
    }

    fn readme(&self, listing: &Listing) -> String {
        let releases_page = format!("https://github.com/{}/releases/latest", listing.repository);
        let shared = AsShared { about: &listing.summary, releases_page: &releases_page, goes_online: !self.streams().is_empty() };
        readme_of(&self.facts, &self.words, &shared)
    }

    fn streams(&self) -> Vec<online::Stream> {
        online::streams_of(&self.project).unwrap_or_default()
    }

    /// A plugin that goes online is handed over with the whole of MP Forge's stream helper, as MP Forge itself
    /// writes it, so that the source in the repository is all of the plugin's source, network code included.
    fn write_what_goes_online(&self, repository: &Path) -> io::Result<()> {
        let streams = self.streams();
        if streams.is_empty() {
            return Ok(());
        }
        online::helper_for(&streams).into_iter().try_for_each(|(place, contents)| write_into(&repository.join(place), contents.as_bytes()))
    }

    fn pages_with_a_picture(&self) -> impl Iterator<Item = (usize, &guide::Page)> {
        let numbered = self.facts.pages.iter().enumerate().map(|(index, page)| (index + 1, page));
        numbered.filter(|(_, page)| !page.picture.is_empty())
    }

    /// The manifest saying what the listing says: Akai's template leaves a line to be filled in where the
    /// description goes, and the catalog shows whatever is there.
    fn manifest_as_listed(&self, manifest: &Json, listing: &Listing) -> String {
        let mut listed = manifest.clone();
        listed.set("description", Json::from(listing.summary.as_str()));
        listed.set("license", Json::from(listing.license.name()));
        listed.set("url", Json::from(format!("https://github.com/{}", listing.repository)));
        listed.written() + "\n"
    }

    // ---------------------------------------------------------------------------------------
    // The release: the built plugin as one zip
    // ---------------------------------------------------------------------------------------

    /// The built folder as the zip the catalog takes: that one folder and nothing beside it.
    fn zipped(&self, listing: &Listing) -> io::Result<Vec<u8>> {
        let top_folder = self.built.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let mut zip = Zip::default();
        for file in files_under(&self.built)? {
            let contents = fs::read(self.built.join(&file))?;
            let listed = if file == Path::new(MANIFEST) { self.built_manifest_as_listed(&contents, listing) } else { contents };
            zip.add(&format!("{top_folder}/{}", written_with_slashes(&file)), &listed)?;
        }
        zip.finish()
    }

    fn built_manifest_as_listed(&self, built_manifest: &[u8], listing: &Listing) -> Vec<u8> {
        match Json::parse(&String::from_utf8_lossy(built_manifest)) {
            Ok(manifest) => self.manifest_as_listed(&manifest, listing).into_bytes(),
            Err(_) => built_manifest.to_vec(),
        }
    }

    // ---------------------------------------------------------------------------------------
    // The catalog: the one file that asks for the plugin to be listed
    // ---------------------------------------------------------------------------------------

    fn catalog_entry(&self, listing: &Listing) -> String {
        let mut entry = Json::object([
            ("id", Json::from(self.id())),
            ("name", Json::from(self.facts.name.as_str())),
            ("author", Json::from(self.facts.maker.as_str())),
            ("repo", Json::from(listing.repository.as_str())),
            ("kind", Json::from(self.facts.kind())),
            ("license", Json::from(listing.license.name())),
            ("summary", Json::from(listing.summary.as_str())),
        ]);
        if let Some((page_number, _)) = self.pages_with_a_picture().next() {
            entry.set("screenshot", Json::from(self.address_in_the_repository(listing, &picture_file_of(page_number))));
        }
        entry.written() + "\n"
    }

    /// Where a file of the repository is read from once the release is published. It names the release's tag,
    /// so the picture on the card stays the one that went with that release.
    fn address_in_the_repository(&self, listing: &Listing, file: &str) -> String {
        format!("https://raw.githubusercontent.com/{}/{}/{file}", listing.repository, self.tag())
    }

    // ---------------------------------------------------------------------------------------
    // The steps: what is left for the musician to do on GitHub
    // ---------------------------------------------------------------------------------------

    fn steps(&self, listing: &Listing, checksum: &str) -> String {
        let (name, repository, license) = (&self.facts.name, &listing.repository, listing.license.name());
        let (zip, id, tag) = (self.zip_name(), self.id(), self.tag());
        let guide = if self.guide_is_written { "a README made from its guide" } else { "a README with its screen and controls" };
        format!(
            "# Listing {name} in the Open MPC catalog\n\n\
             MP Forge made everything in this folder from {name} as its safety check just passed it. Nothing has been sent \
             anywhere. The three steps below are done on GitHub, by you.\n\n\
             The catalog lists open plugins only. Listing {name} makes its code public under the {license} licence, for \
             anyone to read, change and share.\n\n\
             ## 1. Put the plugin's source in your repository\n\n\
             Make a public repository on GitHub named `{repository}`, and upload everything inside the folder \
             `{FOR_THE_REPOSITORY}`. It holds the plugin's code, its screen and pictures, {guide}, and the licence.\n\n\
             ## 2. Publish a release\n\n\
             In that repository, draft a new release. Give it the tag `{tag}` and attach the file in \
             `{FOR_THE_RELEASE}`, which is `{zip}`. Leave the file's name as it is: the catalog finds it by its ending.\n\n\
             ## 3. Ask for it to be listed\n\n\
             The file in `{FOR_THE_CATALOG}`, `{id}.json`, goes into the folder `catalog/plugins` of \
             https://github.com/{THE_CATALOG} as a pull request. That repository is not yours, so GitHub will not let \
             you put a file straight into it. You work in a copy of your own:\n\n\
             1. Open https://github.com/{THE_CATALOG} and press Fork, then Create fork. That makes your own copy of \
             the catalog.\n\
             2. In your copy, open the folder `catalog/plugins`, choose Add file, then Upload files, and drag in \
             `{id}.json`.\n\
             3. Under the file, choose to create a new branch for the commit and start a pull request, and press \
             Propose changes.\n\
             4. On the next page, check that the base repository is `{THE_CATALOG}`, press Create pull request, fill in \
             the form and submit it.\n\n\
             The form asks for the release zip's SHA-256, which is `{checksum}`. It also asks for the output of Open \
             MPC's own check tool. If you cannot run that, say so, and say the zip was made by MP Forge from a plugin \
             built with Akai's SDK.\n\n\
             One of Open MPC's maintainers reads it and merges it. The catalog then fetches your release, checks the \
             zip and lists the plugin, usually by the next day.\n\n\
             ## Before you do\n\n\
             - **Is all of it yours to share?** A remix of somebody else's plugin has to keep that plugin's licence and \
             name its author. Sound files and pictures you added have to be yours to share too.\n\
             - **The name on the card is `{author}`.** It is public.\n\
             - **The line on the card:** {summary}\n\
             {online}\
             - **A later version** needs a higher version number in `plugin.json` and a new release with its own tag. \
             MP Forge does not change the number for you yet.\n\n\
             Open MPC's own instructions are at {HOW_OPEN_MPC_SAYS_IT}\n",
            author = self.facts.maker,
            summary = listing.summary,
            online = self.note_on_going_online(),
        )
    }

    /// The catalog's check marks a plugin that uses the network for a person to look at, so the maker should say why.
    fn note_on_going_online(&self) -> String {
        let streams = self.streams();
        if streams.is_empty() {
            return String::new();
        }
        let addresses: Vec<String> = streams.iter().map(|stream| stream.address()).collect();
        format!(
            "- **This plugin goes online.** Open MPC's check marks a plugin that uses the network for a person to look at. \
             Say in your pull request that it only brings in sound, from: {}\n",
            addresses.join(", ")
        )
    }
}

/// The SHA-256 of a file, written the way checksum tools print it. The catalog's form asks for the zip's.
fn checksum_of(contents: &[u8]) -> String {
    digest(&SHA256, contents).as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn words_joined_by_hyphens(name: &str) -> String {
    name.split(|letter: char| !letter.is_ascii_alphanumeric()).filter(|word| !word.is_empty()).collect::<Vec<&str>>().join("-")
}

fn is_a_version(version: &str) -> bool {
    let numbers: Vec<&str> = version.split('.').collect();
    numbers.len() == 3 && numbers.iter().all(|number| !number.is_empty() && number.chars().all(|digit| digit.is_ascii_digit()))
}

/// Every file under the folder, by its path inside it, in the same order every time. Hidden files are left out.
fn files_under(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(folder)?.filter_map(|entry| entry.ok().map(|entry| PathBuf::from(entry.file_name()))).collect();
    entries.sort();
    for entry in entries.into_iter().filter(|entry| !entry.to_string_lossy().starts_with('.')) {
        let whole = folder.join(&entry);
        if whole.is_dir() {
            found.extend(files_under(&whole)?.into_iter().map(|deeper| entry.join(deeper)));
        } else if fs::symlink_metadata(&whole)?.is_file() {
            found.push(entry);
        }
    }
    Ok(found)
}

fn written_with_slashes(inside_a_folder: &Path) -> String {
    inside_a_folder.iter().map(|part| part.to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn write_into(file: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(file, contents)
}

fn this_year() -> i64 {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0);
    year_of((seconds / SECONDS_IN_A_DAY) as i64)
}

/// The year a day falls in, counting days from the first of January 1970.
fn year_of(days_since_1970: i64) -> i64 {
    let days = days_since_1970 + 719_468;
    let (era, day_of_era) = (days.div_euclid(146_097), days.rem_euclid(146_097));
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let starts_in_march = year_of_era + era * 400;
    let is_january_or_february = day_of_year >= 306;
    if is_january_or_february { starts_in_march + 1 } else { starts_in_march }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn a_listing() -> Listing {
        Listing::given("somebody/tape-echo", License::Mit, "A worn tape echo with wow and flutter.")
    }

    #[test]
    fn a_repositorys_whole_address_is_cut_down_to_owner_and_name() {
        for given in ["somebody/tape-echo", " https://github.com/somebody/tape-echo/ ", "github.com/somebody/tape-echo.git"] {
            assert_eq!(Listing::given(given, License::Mit, "x").repository, "somebody/tape-echo", "`{given}`");
        }
    }

    #[test]
    fn a_listing_the_catalog_would_send_back_says_why() {
        let problems_of = |repository: &str, summary: &str| Listing::given(repository, License::Mit, summary).problems().join(" ");

        assert!(a_listing().problems().is_empty());
        assert!(problems_of("tape-echo", "An echo.").contains("owner/name"));
        assert!(problems_of("some body/tape echo", "An echo.").contains("owner/name"));
        assert!(problems_of("somebody/tape-echo", "  ").contains("needs one line"));
        assert!(problems_of("somebody/tape-echo", &"long ".repeat(80)).contains("too long"));
        assert!(problems_of("somebody/tape-echo", "An echo \u{2014} worn.").contains("plain letters"));
    }

    #[test]
    fn licences_go_by_the_names_the_catalog_knows_and_carry_the_makers_name() {
        assert_eq!(License::named("MIT"), Some(License::Mit));
        assert_eq!(License::named("GPL-3.0-only"), Some(License::Gpl3));
        assert_eq!(License::named("Proprietary"), None);
        assert!(License::Mit.text_for("Somebody", 2026).starts_with("MIT License\n\nCopyright (c) 2026 Somebody\n"));
        assert!(License::Gpl3.text_for("Somebody", 2026).contains("GNU GENERAL PUBLIC LICENSE"));
    }

    #[test]
    fn a_name_becomes_words_joined_by_hyphens() {
        assert_eq!(words_joined_by_hyphens("retro delay"), "retro-delay");
        assert_eq!(words_joined_by_hyphens("  6W6 -- (mk II)! "), "6W6-mk-II");
        assert_eq!(words_joined_by_hyphens("!!!"), "");
    }

    #[test]
    fn a_checksum_is_written_the_way_checksum_tools_print_it() {
        assert_eq!(checksum_of(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn a_version_is_three_numbers() {
        assert!(is_a_version("1.0.0") && is_a_version("12.3.45"));
        assert!(!is_a_version("1.0") && !is_a_version("1.0.0-beta") && !is_a_version("") && !is_a_version("1..0"));
    }

    #[test]
    fn the_year_of_a_day_is_worked_out_from_days_since_1970() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(364), 1970);
        assert_eq!(year_of(365), 1971);
        assert_eq!(year_of(20_453), 2025, "31 December 2025");
        assert_eq!(year_of(20_454), 2026, "1 January 2026");
        assert_eq!(year_of(20_775), 2026, "18 November 2026");
    }

    #[test]
    fn the_first_sentence_of_a_guide_is_suggested_for_the_card() {
        assert_eq!(first_sentence_of("A worn tape echo. It wobbles."), "A worn tape echo.");
        assert_eq!(first_sentence_of("One sentence only."), "One sentence only.");
        assert_eq!(first_sentence_of(""), "");
    }

    #[test]
    fn a_folder_that_is_already_there_is_never_written_over() {
        let into = TemporaryFolder::create("open-mpc-free-folder");
        fs::create_dir_all(into.0.join("Tape Echo for Open MPC")).unwrap();
        fs::create_dir_all(into.0.join("Tape Echo for Open MPC 2")).unwrap();

        assert_eq!(first_free_folder(&into.0, "Tape Echo for Open MPC"), into.0.join("Tape Echo for Open MPC 3"));
        assert_eq!(first_free_folder(&into.0, "Glass Pad for Open MPC"), into.0.join("Glass Pad for Open MPC"));
    }

    #[test]
    fn a_plugin_that_was_never_built_is_not_got_ready() {
        let plugin = TemporaryFolder::create("open-mpc-not-built");
        fs::write(plugin.0.join(MANIFEST), r#"{ "name": "Tape Echo", "manufacturer": "Somebody", "kind": "effect", "version": "1.0.0" }"#).unwrap();
        let into = TemporaryFolder::create("open-mpc-not-built-into");

        assert!(matches!(prepare(&plugin.0, &a_listing(), &into.0), Err(NotListed::NotBuiltYet)));
        assert!(matches!(as_it_would_be_listed(&plugin.0), Err(NotListed::NotBuiltYet)));
        assert_eq!(fs::read_dir(&into.0).unwrap().count(), 0);
    }

    #[test]
    fn files_under_a_folder_are_found_in_the_same_order_without_the_hidden_ones() {
        let folder = TemporaryFolder::create("open-mpc-files-under");
        for file in ["VST/b.so", "VST/a.so", "Plugin Skins/pictures/lamp.png", "plugin.json", ".DS_Store", "VST/.hidden"] {
            write_into(&folder.0.join(file), b"x").unwrap();
        }

        let found: Vec<String> = files_under(&folder.0).unwrap().iter().map(|file| written_with_slashes(file)).collect();

        assert_eq!(found, ["Plugin Skins/pictures/lamp.png", "VST/a.so", "VST/b.so", "plugin.json"]);
    }
}
