//! Validating a record file against its declared shape.
//!
//! Two checks, each driven by a file shipped with the `adr-creator` skill:
//!
//! - the frontmatter, against `schemas/adr-frontmatter.schema.json`;
//! - the body, which must contain every heading `templates/adr.yaml` lists.
//!
//! Prose quality is not checked here. That is `check`'s job, and it scores a
//! record that is already well formed.
//!
//! The schema and heading checks themselves are `pahdk_core::validate`, shared
//! with the other tools; this module supplies the ADR schema and template and
//! the checks that need other records.

use std::fs;
use std::path::Path;

use pahdk_core::frontmatter;
use pahdk_core::validate::{self, Issue};
use pahdk_core::{Error, Result};
use serde::Serialize;
use serde_json::Value;

use crate::{store, template};

/// The embedded frontmatter schema.
const SCHEMA: &str = include_str!("../../../adr-creator/schemas/adr-frontmatter.schema.json");

/// The result of validating one record file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The record's slug.
    pub slug: String,
    /// Empty when the record is valid.
    pub issues: Vec<Issue>,
}

impl Report {
    /// Whether the record has no issues.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Validate the text of one record file.
///
/// The frontmatter check and the heading check are `pahdk_core::validate`'s;
/// the title heading is the one part of the shape that varies per record.
pub fn validate_content(content: &str) -> Result<Vec<Issue>> {
    let schema: Value = serde_json::from_str(SCHEMA)
        .map_err(|e| Error::Config(format!("embedded schema is not JSON: {e}")))?;
    let headings = template::required_headings(&template::load()?);
    let mut issues = validate::document_issues(content, &schema, &headings)?;

    if let Some((_, body)) = frontmatter::split(content) {
        if !body.lines().any(|l| l.starts_with("# ")) {
            issues.push(Issue::new("body", "missing the level-1 title heading"));
        }
    }
    Ok(issues)
}

/// Validate the record with `slug`.
pub fn validate_one(dir: &Path, slug: &str) -> Result<Report> {
    let path = store::path_for(dir, slug);
    if !path.is_file() {
        return Err(Error::NotFound(path));
    }
    let content = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let mut issues = validate_content(&content)?;
    issues.extend(consistency_issues(dir, slug, &content));
    Ok(Report {
        slug: slug.to_string(),
        issues,
    })
}

/// Problems the schema cannot express because they need other files or the
/// relation between two fields: links to records that do not exist, and a
/// status that disagrees with the last recorded transition.
///
/// Silent when the frontmatter does not parse, since `validate_content`
/// already reported that.
fn consistency_issues(dir: &Path, slug: &str, content: &str) -> Vec<Issue> {
    let Some((yaml, _)) = frontmatter::split(content) else {
        return Vec::new();
    };
    let Ok(meta) = serde_yaml_ng::from_str::<Value>(yaml) else {
        return Vec::new();
    };
    let mut issues = Vec::new();

    for field in ["supersedes", "superseded_by"] {
        let Some(target) = meta.get(field).and_then(Value::as_str) else {
            continue;
        };
        let message = if target == slug {
            Some("names the record itself".to_string())
        } else if !store::exists(dir, target) {
            Some(format!(
                "names {target:?}, which has no record in this directory"
            ))
        } else {
            None
        };
        if let Some(message) = message {
            issues.push(Issue {
                location: format!("frontmatter /{field}"),
                message,
            });
        }
    }

    let last = meta
        .get("history")
        .and_then(Value::as_array)
        .and_then(|h| h.last())
        .and_then(|t| t.get("to"))
        .and_then(Value::as_str);
    if let (Some(last), Some(status)) = (last, meta.get("status").and_then(Value::as_str)) {
        if last != status {
            issues.push(Issue {
                location: "frontmatter /status".into(),
                message: format!(
                    "is {status:?} but the last history entry moved the record to {last:?}"
                ),
            });
        }
    }
    issues
}

/// Validate every record in `dir`. A broken record is reported, not fatal.
pub fn validate_all(dir: &Path) -> Result<Vec<Report>> {
    store::slugs(dir)?
        .iter()
        .map(|slug| validate_one(dir, slug))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{BranchType, Record};

    fn fresh() -> String {
        Record::new("x", "Adopt X", BranchType::Feat, "T", "2026-09-15", None)
            .render()
            .unwrap()
    }

    fn messages(content: &str) -> Vec<String> {
        validate_content(content)
            .unwrap()
            .into_iter()
            .map(|i| format!("{}: {}", i.location, i.message))
            .collect()
    }

    #[test]
    fn a_freshly_created_record_is_valid() {
        assert_eq!(messages(&fresh()), Vec::<String>::new());
    }

    #[test]
    fn a_bad_status_is_located_in_the_frontmatter() {
        let bad = fresh().replace("status: proposed", "status: rejected");
        let found = messages(&bad);
        assert!(
            found.iter().any(|m| m.starts_with("frontmatter /status")),
            "{found:?}"
        );
    }

    #[test]
    fn a_missing_required_field_and_an_unknown_one_are_both_reported() {
        let bad = fresh().replace("author: T\n", "colour: red\n");
        let found = messages(&bad).join("\n");
        assert!(found.contains("author"), "{found}");
        assert!(found.contains("colour"), "{found}");
    }

    #[test]
    fn a_superseded_record_must_name_its_replacement() {
        let bad = fresh().replace("status: proposed", "status: superseded");
        assert!(messages(&bad).join("\n").contains("superseded_by"));
        let ok = bad.replace("author: T\n", "author: T\nsuperseded_by: y\n");
        assert!(messages(&ok).is_empty(), "{:?}", messages(&ok));
    }

    #[test]
    fn a_malformed_date_is_rejected() {
        let bad = fresh().replace("2026-09-15", "15/09/2026");
        assert!(messages(&bad)
            .iter()
            .any(|m| m.starts_with("frontmatter /date")));
    }

    #[test]
    fn a_dropped_section_heading_is_reported() {
        let bad = fresh().replace("### Rationale\n", "");
        assert_eq!(
            messages(&bad),
            vec!["body: missing heading \"### Rationale\"".to_string()]
        );
    }

    #[test]
    fn a_file_without_frontmatter_is_reported_once() {
        let issues = validate_content("# Just prose\n").unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location, "frontmatter");
    }

    #[test]
    fn the_schema_enums_match_the_rust_enums() {
        use crate::record::Status;
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        };
        let statuses: Vec<String> = Status::ALL.iter().map(|s| s.as_str().to_string()).collect();
        let types: Vec<String> = BranchType::ALL
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        assert_eq!(strings(&schema["properties"]["status"]["enum"]), statuses);
        assert_eq!(strings(&schema["$defs"]["status"]["enum"]), statuses);
        assert_eq!(strings(&schema["properties"]["branch_type"]["enum"]), types);
    }

    fn on_disk(dir: &Path, slug: &str, content: &str) {
        fs::write(dir.join(format!("{slug}.md")), content).unwrap();
    }

    #[test]
    fn links_to_missing_records_and_self_links_are_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let linked = fresh().replace("author: T\n", "author: T\nsupersedes: ghost\n");
        on_disk(dir, "x", &linked);
        let found = validate_one(dir, "x").unwrap().issues;
        assert!(
            found
                .iter()
                .any(|i| i.location == "frontmatter /supersedes" && i.message.contains("ghost")),
            "{found:?}"
        );

        on_disk(dir, "ghost", &fresh());
        assert!(validate_one(dir, "x").unwrap().is_valid());

        let own = fresh().replace("author: T\n", "author: T\nsupersedes: x\n");
        on_disk(dir, "x", &own);
        let found = validate_one(dir, "x").unwrap().issues;
        assert!(
            found.iter().any(|i| i.message.contains("itself")),
            "{found:?}"
        );
    }

    #[test]
    fn a_status_that_disagrees_with_the_history_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let mut record = Record::new("x", "Adopt X", BranchType::Feat, "T", "2026-09-15", None);
        record.transition(
            crate::record::Status::ReviewRequested,
            "2026-09-16T10:00:00Z",
        );
        on_disk(tmp.path(), "x", &record.render().unwrap());
        assert!(validate_one(tmp.path(), "x").unwrap().is_valid());

        let drifted =
            record
                .render()
                .unwrap()
                .replacen("status: review-requested", "status: accepted", 1);
        on_disk(tmp.path(), "x", &drifted);
        let found = validate_one(tmp.path(), "x").unwrap().issues;
        assert!(
            found
                .iter()
                .any(|i| i.message.contains("last history entry")),
            "{found:?}"
        );
    }
}
