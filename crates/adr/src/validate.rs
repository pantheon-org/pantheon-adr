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
//! [`schema_issues`] is the only code that touches the JSON Schema crate and
//! knows nothing about ADRs, so it can move to a shared crate if another tool
//! needs the same check.

use std::fs;
use std::path::Path;

use pahdk_core::{Error, Result};
use serde::Serialize;
use serde_json::Value;

use crate::{record, store, template};

/// The embedded frontmatter schema.
const SCHEMA: &str = include_str!("../../../adr-creator/schemas/adr-frontmatter.schema.json");

/// One problem found in a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// Where it was found: `frontmatter /status`, `body`, and so on.
    pub location: String,
    /// What is wrong.
    pub message: String,
}

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

/// Every way `instance` fails `schema`, as `(instance location, message)`.
pub fn schema_issues(schema: &Value, instance: &Value) -> Result<Vec<(String, String)>> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| Error::Config(format!("invalid JSON Schema: {e}")))?;
    let mut issues: Vec<(String, String)> = validator
        .iter_errors(instance)
        .map(|e| (e.instance_path().to_string(), e.to_string()))
        .collect();
    issues.sort();
    Ok(issues)
}

/// Validate the text of one record file.
pub fn validate_content(content: &str) -> Result<Vec<Issue>> {
    let Some((yaml, body)) = record::split_frontmatter(content) else {
        return Ok(vec![Issue {
            location: "frontmatter".into(),
            message: "no YAML frontmatter found (expected a leading `---` block)".into(),
        }]);
    };
    let mut issues = Vec::new();

    match serde_yaml_ng::from_str::<Value>(yaml) {
        Err(e) => issues.push(Issue {
            location: "frontmatter".into(),
            message: format!("invalid YAML: {e}"),
        }),
        Ok(instance) => {
            let schema: Value = serde_json::from_str(SCHEMA)
                .map_err(|e| Error::Config(format!("embedded schema is not JSON: {e}")))?;
            for (location, message) in schema_issues(&schema, &instance)? {
                issues.push(Issue {
                    location: format!("frontmatter {location}").trim_end().to_string(),
                    message,
                });
            }
        }
    }

    issues.extend(body_issues(body, &template::load()?));
    Ok(issues)
}

/// A missing title or section heading, as one issue per heading.
fn body_issues(body: &str, template: &template::Template) -> Vec<Issue> {
    let lines: Vec<&str> = body.lines().map(str::trim_end).collect();
    let mut issues = Vec::new();
    if !lines.iter().any(|l| l.starts_with("# ")) {
        issues.push(Issue {
            location: "body".into(),
            message: "missing the level-1 title heading".into(),
        });
    }
    for heading in template::required_headings(template) {
        if !lines.contains(&heading.as_str()) {
            issues.push(Issue {
                location: "body".into(),
                message: format!("missing heading {heading:?}"),
            });
        }
    }
    issues
}

/// Validate the record with `slug`.
pub fn validate_one(dir: &Path, slug: &str) -> Result<Report> {
    let path = store::path_for(dir, slug);
    if !path.is_file() {
        return Err(Error::NotFound(path));
    }
    let content = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    Ok(Report {
        slug: slug.to_string(),
        issues: validate_content(&content)?,
    })
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
}
