# Record Template and Schema

The shape of an ADR is defined by two files in this skill, and the tool reads
both. Neither is hard-coded in the binary's source.

| File | Defines | Used by |
| --- | --- | --- |
| `templates/adr.yaml` | The default frontmatter `type`, and the body: title, section headings, levels and placeholder text | `create` renders it; `validate` requires its headings |
| `schemas/adr-frontmatter.schema.json` | Every frontmatter field, its type, allowed values and which are required (JSON Schema 2020-12) | `validate` |

Both are embedded in the `pantheon-adr` binary at build time, so an installed
binary and a checkout of the skill agree.

## The template

`templates/adr.yaml` lists `body.sections` in order. Each entry has a
`heading`, a `level` (2 for `##`, 3 for `###`) and at most one of:

- `placeholder`: one line written directly under the heading;
- `bullets`: a list written under the heading, after a blank line.

An entry with neither is a parent heading whose children follow it. Changing a
heading changes what `create` writes and what `validate` requires, and it
changes the names `check` scores by, so treat the headings as load-bearing.

## The schema

The schema requires `title`, `type` (always `decision`), `branch_type`,
`status`, `date` (`YYYY-MM-DD`) and `author`. It forbids unknown fields, so a
misspelt `superseeded_by` is reported instead of ignored. A record whose status
is `superseded` must also carry `superseded_by`. A test keeps the schema's
`status` and `branch_type` values equal to the ones the tool writes.

## What `validate` does not check

- Prose quality: use `pantheon-adr check`.
- That `supersedes`, `superseded_by` or `related` point at records or files that
  exist.
- That the status history is consistent with the current status.
