//! End-to-end tests driving the public `adr_core` library the way the CLI
//! does: the full lifecycle of a decision from creation through the
//! completeness gate to supersession, the catalogue derived from it, and
//! installing the embedded skill into a temporary agent directory.

use std::path::Path;

use adr_core::commands::{self, CreateArgs, UpdateArgs};
use adr_core::install_cmd::{run_install, InstallOptions, Selection};
use adr_core::record::Status;
use adr_core::{skill_bundle, store};
use pahdk_install::env::Environment;
use pahdk_install::install::InstallMode;

/// Enough real prose to satisfy the rubric's word-count rules.
const PROSE: &str = "Requests cross four services and we cannot follow a single one end to end \
    because each service writes its own log format with no shared correlation identifier, so \
    reconstructing a slow request means grepping four log stores by timestamp and guessing.";

/// Replace the template body with prose that scores 100.
fn fill(dir: &Path, slug: &str) {
    let mut record = store::load(dir, slug).expect("load the record just created");
    record.body = format!(
        "# {slug}\n\n## Problem Statement\n### Context\n{PROSE}\n\n\
         ## Decision Record\n### Options Considered\nA vendor agent, or OTLP behind a collector.\n\n\
         ### Chosen Solution\n{PROSE}\n\n### Rationale\n{PROSE}\n\n\
         ## Impact Assessment\n- **Performance**: about 1% CPU at 10% sampling\n\
         - **Security**: attributes scrubbed at the collector\n"
    );
    store::save(dir, &record).expect("save the filled record");
}

#[test]
fn the_full_lifecycle_end_to_end() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().join("docs").join("adr");

    commands::init(&dir, tmp.path(), Some("demo"), false, false).expect("init");

    // Create, outside a repository, with an explicit slug.
    let args = CreateArgs {
        slug: Some("adopt-opentelemetry"),
        description: Some("Adopt OpenTelemetry for tracing"),
        ..Default::default()
    };
    commands::create(&dir, tmp.path(), "2026-09-15", &args).expect("create");
    assert!(dir.join("adopt-opentelemetry.md").is_file());

    // An untouched template must not pass the gate.
    let (_, verdict) = commands::check(&dir, "adopt-opentelemetry", false, false).expect("check");
    assert!(verdict.is_err(), "the template must not score as finished");
    assert!(
        commands::review(&dir, "adopt-opentelemetry", "2026-09-15T10:00:00Z").is_err(),
        "review must refuse an unfinished record"
    );

    // Fill it in, and the gate opens.
    fill(&dir, "adopt-opentelemetry");
    let (report, verdict) =
        commands::check(&dir, "adopt-opentelemetry", false, false).expect("check");
    assert!(verdict.is_ok(), "{report}");
    assert!(report.contains("Score: 100/100"), "{report}");

    commands::review(&dir, "adopt-opentelemetry", "2026-09-15T10:00:00Z").expect("review");
    commands::update(
        &dir,
        "adopt-opentelemetry",
        "2026-09-15T11:00:00Z",
        &UpdateArgs {
            status: Some("accepted"),
            tags: Some("tracing,observability"),
            ..Default::default()
        },
    )
    .expect("accept");

    let accepted = store::load(&dir, "adopt-opentelemetry").expect("load");
    assert_eq!(accepted.meta.status, Status::Accepted);
    assert_eq!(accepted.meta.tags, vec!["tracing", "observability"]);
    assert_eq!(
        accepted.meta.history.len(),
        2,
        "proposed to review-requested to accepted"
    );

    // Supersede it with a second decision; both ends of the chain are linked.
    let replacement = CreateArgs {
        slug: Some("adopt-grafana-tempo"),
        description: Some("Adopt Grafana Tempo as the tracing backend"),
        ..Default::default()
    };
    commands::create(&dir, tmp.path(), "2026-09-16", &replacement).expect("create replacement");
    commands::update(
        &dir,
        "adopt-opentelemetry",
        "2026-09-16T09:00:00Z",
        &UpdateArgs {
            superseded_by: Some("adopt-grafana-tempo"),
            ..Default::default()
        },
    )
    .expect("supersede");

    let old = store::load(&dir, "adopt-opentelemetry").expect("load old");
    assert_eq!(old.meta.status, Status::Superseded);
    assert_eq!(
        old.meta.superseded_by.as_deref(),
        Some("adopt-grafana-tempo")
    );
    let new = store::load(&dir, "adopt-grafana-tempo").expect("load new");
    assert_eq!(new.meta.supersedes.as_deref(), Some("adopt-opentelemetry"));

    // The prose of the superseded record is untouched by any of that.
    assert!(
        old.body.contains(PROSE),
        "superseding must not rewrite the decision text"
    );

    // Listing, filtering and the catalogue all agree.
    let listed = commands::list(&dir, None, None, false).expect("list");
    assert!(listed.contains("2 ADR(s)"), "{listed}");
    let superseded = commands::list(&dir, Some("superseded"), None, false).expect("list");
    assert!(superseded.contains("adopt-opentelemetry"), "{superseded}");

    commands::index_cmd(&dir, "2026-09-16").expect("index");
    let catalogue = std::fs::read_to_string(dir.join("index.md")).expect("catalogue");
    assert!(catalogue.contains("## Superseded"), "{catalogue}");
    assert!(
        catalogue.contains("superseded by `adopt-grafana-tempo`"),
        "{catalogue}"
    );

    // The catalogue is not itself a record.
    assert_eq!(store::load_all(&dir).expect("load all").len(), 2);
}

#[test]
fn status_json_is_stable_for_a_fixed_collection() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().join("docs").join("adr");
    commands::init(&dir, tmp.path(), Some("demo"), false, false).expect("init");

    for slug in ["one", "two"] {
        let args = CreateArgs {
            slug: Some(slug),
            ..Default::default()
        };
        commands::create(&dir, tmp.path(), "2026-09-15", &args).expect("create");
    }

    let first = commands::status(&dir, tmp.path(), true).expect("status");
    let second = commands::status(&dir, tmp.path(), true).expect("status");
    assert_eq!(first, second, "JSON output must be deterministic");

    let value: serde_json::Value = serde_json::from_str(&first).expect("valid json");
    assert_eq!(value["total"], 2);
    assert_eq!(value["by_status"]["proposed"], 2);
}

#[test]
fn a_record_round_trips_through_the_filesystem_unchanged() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().join("docs").join("adr");
    let args = CreateArgs {
        slug: Some("round-trip"),
        description: Some("Round trip"),
        ..Default::default()
    };
    commands::create(&dir, tmp.path(), "2026-09-15", &args).expect("create");

    let loaded = store::load(&dir, "round-trip").expect("load");
    let rendered = loaded.render().expect("render");
    let reloaded = adr_core::record::Record::parse("round-trip", &rendered).expect("parse");
    assert_eq!(reloaded, loaded);
}

#[test]
fn skill_installs_into_a_temp_agent_dir() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let env = Environment::resolve(home.clone(), home.clone(), None, None, None);

    // Extract the embedded skill, then install it into an explicit agent.
    let bundle_root = tmp.path().join("bundle");
    let source = skill_bundle::materialise(&bundle_root).expect("materialise the bundled skill");
    assert!(source.join("SKILL.md").is_file());

    let opts = InstallOptions {
        selection: Selection::Explicit(vec!["claude-code".into()]),
        global: true,
        mode: InstallMode::Copy,
        dry_run: false,
    };
    let exists = |p: &Path| p.exists();
    let report = run_install(&opts, &env, &exists, &source).expect("install");

    assert_eq!(report.outcomes.len(), 1);
    let installed = report.outcomes[0]
        .installed_path
        .as_ref()
        .expect("an installed path");
    assert!(installed.join("SKILL.md").is_file());
    assert_eq!(
        installed.file_name().expect("a directory name"),
        "adr-creator"
    );
}

#[test]
fn undocumented_reports_only_unlinked_decisions() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();
    let adr = root.join("docs").join("adr");
    let ctx = root.join(".context");
    std::fs::create_dir_all(&ctx).unwrap();

    std::fs::write(
        ctx.join("spike.md"),
        "# Spike\n\n## Decision\nWe will use Redis.\n",
    )
    .unwrap();
    std::fs::write(ctx.join("linked.md"), "## Decision\nAdopt Option B.\n").unwrap();
    std::fs::write(ctx.join("notes.md"), "# Notes\nLatency is 40ms.\n").unwrap();

    commands::init(&adr, root, Some("demo"), false, false).expect("init");
    let args = CreateArgs {
        slug: Some("adopt-b"),
        description: Some("Adopt B"),
        ..Default::default()
    };
    commands::create(&adr, root, "2026-09-15", &args).expect("create");
    let mut record = store::load(&adr, "adopt-b").unwrap();
    record.meta.related = vec!["../../.context/linked.md".to_string()];
    store::save(&adr, &record).unwrap();

    let (text, found) =
        commands::undocumented_cmd(Path::new("docs/adr"), root, None, false).expect("scan");
    assert!(found);
    assert!(text.contains(".context/spike.md"), "{text}");
    assert!(text.contains("Indicator: ## Decision"), "{text}");
    assert!(!text.contains("linked.md"), "{text}");
    assert!(!text.contains("notes.md"), "{text}");
    assert!(text.contains("Total: 1 undocumented decision(s)"), "{text}");

    let (json, _) =
        commands::undocumented_cmd(Path::new("docs/adr"), root, None, true).expect("json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["undocumented"][0]["path"], ".context/spike.md");

    // Linking the spike clears the finding.
    record
        .meta
        .related
        .push("../../.context/spike.md".to_string());
    store::save(&adr, &record).unwrap();
    let (text, found) =
        commands::undocumented_cmd(Path::new("docs/adr"), root, None, false).expect("rescan");
    assert!(!found, "{text}");

    // A missing source directory is not an error.
    let (text, found) =
        commands::undocumented_cmd(Path::new("docs/adr"), root, Some("nope"), false).unwrap();
    assert!(!found && text.contains("nothing to scan"), "{text}");
}

#[test]
fn every_record_the_tool_writes_passes_validate() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().join("docs").join("adr");
    commands::init(&dir, tmp.path(), Some("demo"), false, false).expect("init");

    for slug in ["first", "second"] {
        let args = CreateArgs {
            slug: Some(slug),
            description: Some("A decision"),
            ..Default::default()
        };
        commands::create(&dir, tmp.path(), "2026-09-15", &args).expect("create");
    }
    // Fill every placeholder in place, keeping every heading.
    let mut record = store::load(&dir, "first").expect("load");
    record.body = record
        .body
        .replace("<!-- impact or \"none\" -->", "about 1% CPU");
    while let Some(start) = record.body.find("<!--") {
        let end = start + record.body[start..].find("-->").expect("closed comment") + 3;
        record.body.replace_range(start..end, PROSE);
    }
    store::save(&dir, &record).expect("save");
    commands::review(&dir, "first", "2026-09-16T10:00:00Z").expect("review");
    commands::update(
        &dir,
        "first",
        "2026-09-17T10:00:00Z",
        &UpdateArgs {
            superseded_by: Some("second"),
            tags: Some("tracing, otel"),
            ..Default::default()
        },
    )
    .expect("supersede");

    let (text, verdict) = commands::validate_cmd(&dir, None, false).expect("validate");
    assert!(verdict.is_ok(), "{text}");
    assert!(text.contains("2 records checked, 0 invalid"), "{text}");

    // Break one record and the verdict turns, naming the field.
    let path = dir.join("second.md");
    let broken = std::fs::read_to_string(&path)
        .unwrap()
        .replace("status: proposed", "status: rejected");
    std::fs::write(&path, broken).unwrap();

    let (text, verdict) = commands::validate_cmd(&dir, None, false).expect("validate");
    assert!(verdict.is_err(), "{text}");
    assert!(text.contains("FAIL  second"), "{text}");
    assert!(text.contains("frontmatter /status"), "{text}");
    assert!(text.contains("ok    first"), "{text}");

    let (json, _) = commands::validate_cmd(&dir, Some("second"), true).expect("json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["invalid"], 1);

    assert!(commands::validate_cmd(&dir, Some("missing"), false).is_err());
}

#[test]
fn update_refuses_a_superseded_status_without_a_replacement() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().join("adr");
    for slug in ["old", "new"] {
        let args = CreateArgs {
            slug: Some(slug),
            description: Some("A decision"),
            ..Default::default()
        };
        commands::create(&dir, tmp.path(), "2026-09-15", &args).expect("create");
    }
    let bare = UpdateArgs {
        status: Some("superseded"),
        ..Default::default()
    };
    let err = commands::update(&dir, "old", "2026-09-16T10:00:00Z", &bare)
        .expect_err("a bare superseded status must be refused")
        .to_string();
    assert!(err.contains("--superseded-by"), "{err}");
    assert_eq!(
        store::load(&dir, "old").unwrap().meta.status,
        Status::Proposed
    );

    // With a replacement, either spelling works and the record validates.
    let both = UpdateArgs {
        status: Some("superseded"),
        superseded_by: Some("new"),
        ..Default::default()
    };
    commands::update(&dir, "old", "2026-09-16T10:00:00Z", &both).expect("supersede");
    let (text, verdict) = commands::validate_cmd(&dir, None, false).expect("validate");
    assert!(verdict.is_ok(), "{text}");
}
