# AGENTS.md

Pointers to the files that matter. Read the one a task needs, when it needs it.

## The project

- [`README.md`](README.md): what MakerPnP is, its tools, how to build and run them, and the Crux architecture shared by
  the planner's core and its front ends.
- [`assets/repo-layout.md`](assets/repo-layout.md): where things are, where the layout isn't Cargo's convention.
- [`assets/known-issues.md`](assets/known-issues.md): the index of each tool's known issues.

## The planner

- [`assets/specs/planner/README.md`](assets/specs/planner/README.md): the planner's requirement specifications, their
  conventions, and the change log. Read the area's spec before changing the planner's behaviour; the specs on disk are
  authoritative, and tests cite their requirement IDs.
- [`assets/specs/planner/known-issues.md`](assets/specs/planner/known-issues.md): the planner's known issues, with
  workarounds. Check it before investigating a bug, as one may already describe it.
- [`planning/planner_cli/tests/planner.rs`](planning/planner_cli/tests/planner.rs): the CLI's `operation_sequence_1`,
  which the GUI's workflow tests mirror step by step.
- [`planning/planner_gui_egui/tests/ui/`](planning/planner_gui_egui/tests/ui/): the GUI's tests, run against the whole
  application with no window. `fixture.rs` has the helpers, `workflow.rs` the shared steps that build up a project.

## Code conventions

- [`Hacks.md`](Hacks.md): what each `HACK:` comment in the code means, before changing code near one.
- [`.rustfmt.toml`](.rustfmt.toml): formatting, run with `cargo +nightly fmt`.

## Checks

- [`support/git/hooks/`](support/git/hooks/): the repo's git hooks; `pre-push` checks formatting.
- [`.github/workflows/rust.yml`](.github/workflows/rust.yml): what CI builds and tests.
