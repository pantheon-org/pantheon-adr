//! The ADR template: the shape of a new record's body.
//!
//! The shape lives in `skills/adr-creator/assets/adr.yaml`, embedded at compile
//! time, so the skill documents it and the tool renders it from one source.
//! [`render`] builds a new record's body and [`required_headings`] lists what
//! `validate` expects to find in any record.

use pahdk_core::{Error, Result};
use serde::Deserialize;

/// The embedded template source.
const SOURCE: &str = include_str!("../../../skills/adr-creator/assets/adr.yaml");

/// The parsed template.
#[derive(Debug, Clone, Deserialize)]
pub struct Template {
    /// Defaults a new record's frontmatter starts with.
    pub frontmatter: Defaults,
    /// The body layout.
    pub body: Body,
}

/// Frontmatter defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct Defaults {
    /// The `type` value every record carries.
    #[serde(rename = "type")]
    pub doc_type: String,
}

/// The body layout.
#[derive(Debug, Clone, Deserialize)]
pub struct Body {
    /// The level-1 title line, with `{title}` replaced by the record's title.
    pub title: String,
    /// The sections, in order.
    pub sections: Vec<Section>,
}

/// One heading and what sits under it.
#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    /// The heading text.
    pub heading: String,
    /// The heading level: 2 for `##`, 3 for `###`.
    pub level: usize,
    /// A line written directly under the heading.
    #[serde(default)]
    pub placeholder: Option<String>,
    /// Lines written under the heading, after a blank line.
    #[serde(default)]
    pub bullets: Vec<String>,
}

impl Section {
    /// The heading line, such as `### Context`.
    pub fn heading_line(&self) -> String {
        format!("{} {}", "#".repeat(self.level), self.heading)
    }
}

/// Parse the embedded template.
pub fn load() -> Result<Template> {
    parse(SOURCE)
}

/// Parse template source, rejecting a shape the renderer cannot honour.
pub fn parse(source: &str) -> Result<Template> {
    let template: Template = serde_yaml_ng::from_str(source)
        .map_err(|e| Error::Config(format!("invalid ADR template: {e}")))?;
    for section in &template.body.sections {
        if !(2..=6).contains(&section.level) {
            return Err(Error::Config(format!(
                "invalid ADR template: {:?} has level {}, expected 2 to 6",
                section.heading, section.level
            )));
        }
        if section.placeholder.is_some() && !section.bullets.is_empty() {
            return Err(Error::Config(format!(
                "invalid ADR template: {:?} sets both placeholder and bullets",
                section.heading
            )));
        }
    }
    Ok(template)
}

/// Render a new record's body for `title`.
pub fn render(template: &Template, title: &str) -> String {
    let mut blocks = vec![format!(
        "# {}\n",
        template.body.title.replace("{title}", title)
    )];
    for section in &template.body.sections {
        let mut block = format!("{}\n", section.heading_line());
        if let Some(placeholder) = &section.placeholder {
            block.push_str(placeholder);
            block.push('\n');
        } else if !section.bullets.is_empty() {
            block.push('\n');
            for line in &section.bullets {
                block.push_str(line);
                block.push('\n');
            }
        }
        blocks.push(block);
    }
    blocks.join("\n")
}

/// Every heading line a record body must contain.
pub fn required_headings(template: &Template) -> Vec<String> {
    template
        .body
        .sections
        .iter()
        .map(Section::heading_line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_template_parses() {
        let template = load().expect("embedded template is valid");
        assert_eq!(template.frontmatter.doc_type, "decision");
        assert!(required_headings(&template).contains(&"### Chosen Solution".to_string()));
    }

    #[test]
    fn rendering_substitutes_the_title_and_orders_sections() {
        let body = render(&load().unwrap(), "Adopt X");
        assert!(body.starts_with("# Adopt X\n\n## Problem Statement\n\n### Context\n<!--"));
        assert!(body.ends_with("lessons learned -->\n"));
    }

    #[test]
    fn a_section_with_placeholder_and_bullets_is_rejected() {
        let source = "frontmatter: {type: decision}\nbody:\n  title: t\n  sections:\n    - {heading: A, level: 2, placeholder: p, bullets: ['- x']}\n";
        assert!(parse(source).is_err());
    }

    #[test]
    fn a_level_outside_two_to_six_is_rejected() {
        let source = "frontmatter: {type: decision}\nbody:\n  title: t\n  sections:\n    - {heading: A, level: 1}\n";
        assert!(parse(source).is_err());
    }

    #[test]
    fn the_template_has_a_heading_for_every_section_the_rubric_scores() {
        use crate::completeness::{
            IMPACT_SECTION, PROBLEM_SECTIONS, RATIONALE_SECTION, SOLUTION_SECTION,
        };
        let template = load().unwrap();
        let headings: Vec<&str> = template
            .body
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        let scored = PROBLEM_SECTIONS.iter().copied().chain([
            SOLUTION_SECTION,
            RATIONALE_SECTION,
            IMPACT_SECTION,
        ]);
        for name in scored {
            assert!(
                headings.iter().any(|h| h.contains(name)),
                "the template has no heading containing {name:?}, which `check` scores"
            );
        }
    }
}
