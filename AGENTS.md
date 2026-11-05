# AGENTS.md

Pointers to the files that matter. Read the one a task needs, when it needs it.

## The project

- [`README.md`](README.md): what MakerPnP is, its tools, how to build and run them, and the Crux architecture shared by
  the planner's core and its front ends.
- [`assets/pcb-assembly.md`](assets/pcb-assembly.md): the MakerPnP tools, each with its own name, and the assembly
  domain: how components get attached (hand soldering, paste and reflow, glue and wave, straddle-mount, other machines)
  and what a pick-and-place machine does. Read it before
  reasoning about processes, operations, tasks or phases.
- [`assets/repo-layout.md`](assets/repo-layout.md): where things are, where the layout isn't Cargo's convention.
- [`assets/specs/issues/README.md`](assets/specs/issues/README.md): every tool's open issues, one file each, with steps to
  repeat them. Check them before changing an area's behaviour, a refactor included, as one may already describe the
  code you're changing. Keep an issue's steps to repeat up to date when a change affects them, and follow them before
  and after fixing it. A fixed issue's file is deleted and its description moves to a comment on its regression test,
  "Regression test for `ISSUE-<n>`": search the code for that when investigating a bug, as it may have been fixed
  before.
- [`assets/missions/`](assets/missions/): the missions, one file each: a goal bigger than one change, its high-level
  requirements and its steps. Read the mission a task is part of before starting it, and keep its steps up to date.
  - [`variant-builder.md`](assets/missions/variant-builder.md): the Variant Builder in the Planner's GUI, and data
    sources (parts, substitutions and so on) as top-level objects shared through a core.

## The planner

- [`assets/specs/planner/README.md`](assets/specs/planner/README.md): the planner's requirement specifications, their
  conventions, and the change log. Read the area's spec before changing the planner's behaviour; the specs on disk are
  authoritative, and tests cite their requirement IDs. Its "Principles" say how the core and the front ends share the
  work: read them before adding a core event.
- [`assets/specs/planner/core/03-logging.md`](assets/specs/planner/core/03-logging.md): what is logged, and how to capture
  the logs, `--trace <file> -v` for the CLI and the GUI. Use the logs to observe an issue, and check the log lines in
  order in tests, as `assert_contains_inorder!` and the GUI fixture's `wait_for_logs_in_order` do.
- [`planning/planner_cli/tests/planner.rs`](planning/planner_cli/tests/planner.rs): the CLI's `operation_sequence_1`,
  which the GUI's workflow tests mirror step by step.
- [`planning/planner_gui_egui/tests/ui/`](planning/planner_gui_egui/tests/ui/): the GUI's tests, run against the whole
  application with no window. `fixture.rs` has the helpers, `workflow.rs` the shared steps that build up a project.

## Code conventions

- [`Hacks.md`](Hacks.md): what each `HACK:` comment in the code means, before changing code near one.
- [`.rustfmt.toml`](.rustfmt.toml): formatting, run with `cargo +nightly fmt`.
- Line endings: keep each file's own, CRLF or LF, as a file whose endings change shows every line as changed in review.
  Leave `.gitattributes` and `newline_style` as they are. Change files with a tool that can be made to keep a file's
  endings, such as a script that reads them and writes them back, and avoid one that can't, such as `sed`, a `perl`
  one-liner, a heredoc, or writing a whole file over an existing one. Even an edit tool can change them, so note a
  file's endings with `git ls-files --eol <file>` before changing it, and check them again after; put back any that
  changed before finishing.

## Checks

- [`support/git/hooks/`](support/git/hooks/): the repo's git hooks; `pre-push` checks formatting.
- [`.github/workflows/rust.yml`](.github/workflows/rust.yml): what CI checks, builds and tests.
- `clippy.toml` in each core's crate: the cores do no IO; the stores service does it.

## Scripts

- [`support/scripts/`](support/scripts/): Rust is the only language to expect installed. Write scripted work, bulk
  edits included, as a Rust cargo script here, rather than with bash, sed, perl, python or one-off edits, using `clap`
  so `--help` documents it for humans and agents to reuse. Run one with
  `cargo +nightly -Zscript support/scripts/<name>.rs --help`; check for one before writing your own.
