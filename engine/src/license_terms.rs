//! What a licence lets a musician do with somebody else's plugin, as far as MP Forge needs to know.
//!
//! Every open source licence lets anyone take the code, build it, use it and change it. They differ in what
//! is owed once a changed copy is shared: some ask only that the maker's notice stays on it, others that it
//! stays open under the same licence. A licence MP Forge does not know is treated as giving no leave at all.

const WORDS_BETWEEN_LICENSES: [&str; 2] = [" AND ", " OR "];
const ASK_ONLY_FOR_THE_NOTICE: [&str; 5] = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"];
const FAMILIES_THAT_STAY_THE_SAME: [&str; 3] = ["GPL-", "AGPL-", "LGPL-"];

/// What is owed when a changed copy is shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terms {
    /// The original's copyright notice and licence text go along with it.
    KeepTheNotice,
    /// It stays under the original's licence, with its source.
    StaysTheSame,
}

/// The terms of a licence as lists name it, such as `MIT` or `GPL-3.0-only`. A plugin under several licences
/// at once is held to the strictest of them. None for a licence that is not known to be open source.
pub fn terms_of(license: &str) -> Option<Terms> {
    let each: Option<Vec<Terms>> = parts_of(license).into_iter().map(terms_of_one).collect();
    let each = each.filter(|terms| !terms.is_empty())?;
    Some(if each.contains(&Terms::StaysTheSame) { Terms::StaysTheSame } else { Terms::KeepTheNotice })
}

/// The terms in a sentence a musician can act on, for a licence whose terms are known.
pub fn in_plain_words(license: &str) -> Option<String> {
    Some(match terms_of(license)? {
        Terms::KeepTheNotice => format!("{license}: you can use it and change it. If you share your version, keep the original maker's notice and licence with it."),
        Terms::StaysTheSame => format!("{license}: you can use it and change it. If you share your version, it has to stay under {license}, with its source."),
    })
}

fn parts_of(license: &str) -> Vec<&str> {
    let mut parts = vec![license];
    for between in WORDS_BETWEEN_LICENSES {
        parts = parts.into_iter().flat_map(|part| part.split(between)).collect();
    }
    parts.into_iter().map(str::trim).collect()
}

fn terms_of_one(license: &str) -> Option<Terms> {
    if ASK_ONLY_FOR_THE_NOTICE.contains(&license) {
        return Some(Terms::KeepTheNotice);
    }
    FAMILIES_THAT_STAY_THE_SAME.iter().any(|family| license.starts_with(family)).then_some(Terms::StaysTheSame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_licence_that_asks_for_the_notice_is_told_from_one_that_stays_the_same() {
        assert_eq!(terms_of("MIT"), Some(Terms::KeepTheNotice));
        assert_eq!(terms_of("Apache-2.0"), Some(Terms::KeepTheNotice));
        assert_eq!(terms_of("GPL-3.0-only"), Some(Terms::StaysTheSame));
        assert_eq!(terms_of("GPL-2.0-or-later"), Some(Terms::StaysTheSame));
        assert_eq!(terms_of("AGPL-3.0-or-later"), Some(Terms::StaysTheSame));
    }

    #[test]
    fn a_plugin_under_several_licences_is_held_to_the_strictest() {
        assert_eq!(terms_of("MIT AND Apache-2.0"), Some(Terms::KeepTheNotice));
        assert_eq!(terms_of("GPL-3.0-only AND AGPL-3.0-or-later"), Some(Terms::StaysTheSame));
        assert_eq!(terms_of("MIT OR GPL-3.0-only"), Some(Terms::StaysTheSame));
    }

    #[test]
    fn a_licence_that_is_not_known_to_be_open_source_gives_no_leave() {
        assert_eq!(terms_of("PolyForm-Noncommercial-1.0.0"), None);
        assert_eq!(terms_of("MIT AND PolyForm-Shield-1.0.0"), None);
        assert_eq!(terms_of("LicenseRef-LXR02-Open"), None);
        assert_eq!(terms_of(""), None);
        assert_eq!(in_plain_words("PolyForm-Noncommercial-1.0.0"), None);
    }

    #[test]
    fn the_terms_are_said_in_a_sentence_that_names_the_licence() {
        assert_eq!(in_plain_words("MIT").unwrap(), "MIT: you can use it and change it. If you share your version, keep the original maker's notice and licence with it.");
        assert_eq!(in_plain_words("GPL-3.0-only").unwrap(), "GPL-3.0-only: you can use it and change it. If you share your version, it has to stay under GPL-3.0-only, with its source.");
    }
}
