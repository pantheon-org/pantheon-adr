//! Finding planning documents that hold a decision no record points back at.
//!
//! A planning document (a design note, a spike write-up, a review) is
//! *undocumented* when it contains a decision-shaped signal and no record lists
//! it under its frontmatter `related` key. The signals are the ones in the
//! skill's `references/context-extraction.md` ("Recognising a binding
//! decision"); keep the two in step when either changes.
//!
//! Records are read with [`crate::store`], so the `related` lists come from the
//! same frontmatter parser every other command uses.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use pahdk_core::{Error, Result};
use serde::Serialize;

use crate::store;

/// Where planning documents live unless told otherwise.
pub const DEFAULT_SOURCE_DIR: &str = ".context";

/// A decision-shaped signal and the label reported when it matches.
#[derive(Clone, Copy)]
enum Signal {
    /// A heading line: `marker`, optional spaces, then `text` as a whole word.
    Heading(&'static str, &'static str),
    /// A phrase anywhere in the text, matching on word boundaries.
    Phrase(&'static str),
}

/// Checked in this order; the first match is the one reported.
const SIGNALS: [Signal; 10] = [
    Signal::Heading("##", "Decision"),
    Signal::Heading("###", "Chosen Solution"),
    Signal::Heading("##", "Recommendation"),
    Signal::Heading("##", "Recommended Approach"),
    Signal::Heading("##", "Proposed Approach"),
    Signal::Phrase("**Decision:**"),
    Signal::Phrase("**Recommendation:**"),
    Signal::Phrase("We will"),
    Signal::Phrase("Adopt Option"),
    Signal::Phrase("Going forward, we"),
];

impl Signal {
    /// The words a reader would look for, without markdown markers.
    #[cfg(test)]
    fn words(self) -> &'static str {
        match self {
            Signal::Heading(_, text) => text,
            Signal::Phrase(text) => text.trim_matches(|c| c == '*' || c == ':'),
        }
    }

    /// The label shown to the user.
    fn label(self) -> String {
        match self {
            Signal::Heading(marker, text) => format!("{marker} {text}"),
            Signal::Phrase(text) => text.to_string(),
        }
    }

    fn matches(self, content: &str) -> bool {
        match self {
            Signal::Heading(marker, text) => content
                .lines()
                .any(|line| heading_matches(line, marker, text)),
            Signal::Phrase(text) => phrase_matches(content, text),
        }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `marker`, optional whitespace, then `text` ending on a word boundary.
fn heading_matches(line: &str, marker: &str, text: &str) -> bool {
    let Some(rest) = line.strip_prefix(marker) else {
        return false;
    };
    let Some(rest) = rest.trim_start().strip_prefix(text) else {
        return false;
    };
    // `###` must not satisfy a `##` marker: the char after the marker is `#`,
    // which `trim_start` leaves in place, so `strip_prefix(text)` already fails.
    !rest.starts_with(is_word_char)
}

/// Whether `phrase` occurs in `content` with a word boundary at each end where
/// the phrase itself starts or ends with a word character.
fn phrase_matches(content: &str, phrase: &str) -> bool {
    let first_is_word = phrase.starts_with(is_word_char);
    let last_is_word = phrase.ends_with(is_word_char);
    content.match_indices(phrase).any(|(start, _)| {
        let before_ok = !first_is_word || !content[..start].ends_with(is_word_char);
        let after_ok = !last_is_word || !content[start + phrase.len()..].starts_with(is_word_char);
        before_ok && after_ok
    })
}

/// The first decision signal in `content`, if any.
pub fn find_signal(content: &str) -> Option<String> {
    SIGNALS
        .iter()
        .find(|signal| signal.matches(content))
        .map(|signal| signal.label())
}

/// A planning document with a decision signal and no record pointing at it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Path relative to the scan root, with `/` separators.
    pub path: String,
    /// The first signal that matched.
    pub indicator: String,
}

/// The outcome of a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scan {
    /// Whether the source directory existed.
    pub scanned: bool,
    /// Documents with a decision signal and no record pointing at them.
    pub undocumented: Vec<Finding>,
}

/// Every existing path the records list under `related`, canonicalised.
fn referenced(adr_dir: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut seen = BTreeSet::new();
    for record in store::load_all(adr_dir)? {
        for entry in &record.meta.related {
            if let Ok(path) = fs::canonicalize(adr_dir.join(entry)) {
                seen.insert(path);
            }
        }
    }
    Ok(seen)
}

/// Every `*.md` file under `dir`, sorted. Symlinks are not followed.
fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| Error::io(dir, e))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::io(dir, e))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| Error::io(&path, e))?;
        if kind.is_dir() {
            markdown_files(&path, out)?;
        } else if kind.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

/// Scan `source_dir` for planning documents with an unlinked decision.
///
/// `adr_dir` and `source_dir` are resolved against `root`, which the CLI sets to
/// the working directory, as for every other command. Documents inside
/// the ADR directory are the records themselves and are never reported.
pub fn scan(root: &Path, adr_dir: &Path, source_dir: &Path) -> Result<Scan> {
    let adr_dir = root.join(adr_dir);
    let source_dir = root.join(source_dir);
    if !source_dir.is_dir() {
        return Ok(Scan {
            scanned: false,
            undocumented: Vec::new(),
        });
    }

    let linked = referenced(&adr_dir)?;
    let adr_canonical = fs::canonicalize(&adr_dir).ok();
    let root_canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    let mut files = Vec::new();
    markdown_files(&source_dir, &mut files)?;

    let mut undocumented = Vec::new();
    for file in files {
        let resolved = fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
        if linked.contains(&resolved) {
            continue;
        }
        if adr_canonical
            .as_ref()
            .is_some_and(|adr| resolved.starts_with(adr))
        {
            continue;
        }
        let bytes = fs::read(&file).map_err(|e| Error::io(&file, e))?;
        let content = String::from_utf8_lossy(&bytes);
        if let Some(indicator) = find_signal(&content) {
            let relative = resolved.strip_prefix(&root_canonical).unwrap_or(&resolved);
            undocumented.push(Finding {
                path: relative
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
                indicator,
            });
        }
    }
    Ok(Scan {
        scanned: true,
        undocumented,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_signals_need_the_exact_level_and_a_word_boundary() {
        assert_eq!(find_signal("## Decision\nbody"), Some("## Decision".into()));
        assert_eq!(find_signal("##Decision"), Some("## Decision".into()));
        assert_eq!(find_signal("## Decision log"), Some("## Decision".into()));
        assert_eq!(find_signal("## Decisions"), None);
        assert_eq!(find_signal("### Decision"), None);
        assert_eq!(
            find_signal("### Chosen Solution"),
            Some("### Chosen Solution".into())
        );
        assert_eq!(
            find_signal("  ## Decision"),
            None,
            "headings start the line"
        );
    }

    #[test]
    fn phrase_signals_match_on_word_boundaries() {
        assert_eq!(
            find_signal("Then we will ship."),
            None,
            "matching is case sensitive"
        );
        assert_eq!(find_signal("So We will use Redis."), Some("We will".into()));
        assert_eq!(find_signal("Newe will"), None);
        assert_eq!(find_signal("We willingly"), None);
        assert_eq!(
            find_signal("**Decision:** use Redis"),
            Some("**Decision:**".into())
        );
        assert_eq!(
            find_signal("Going forward, we log"),
            Some("Going forward, we".into())
        );
    }

    #[test]
    fn the_first_signal_in_list_order_is_reported() {
        assert_eq!(
            find_signal("We will\n## Decision"),
            Some("## Decision".into())
        );
    }

    #[test]
    fn plain_observation_has_no_signal() {
        assert_eq!(find_signal("# Notes\n\nLatency is 40ms at p99.\n"), None);
    }

    #[test]
    fn every_signal_is_documented_in_the_skill_reference() {
        let doc = include_str!("../../../adr-creator/references/context-extraction.md");
        for signal in SIGNALS {
            assert!(
                doc.contains(signal.words()),
                "context-extraction.md does not mention the signal {:?}",
                signal.label()
            );
        }
    }
}
