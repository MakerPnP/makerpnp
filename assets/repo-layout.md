# Repository layout

The parts of the layout that Cargo's conventions don't explain. The workspace's crates are listed in the root
`Cargo.toml`.

## Crates

- Crates are grouped by area in top-level folders: `common/`, `eda/`, `gerber/`, `pnp/` and `planning/`.
- In `planning/`, a tool's `*_app` crate is its headless Crux core, for example `planner_app`, and its front ends are
  separate crates using that core, for example `planner_cli` and `planner_gui_egui`.

## Files that aren't code

- `assets/`: files for the whole repository.
  - `specs/`: requirement specifications, a folder per tool, each with its own `README.md`.
  - `specs/issues/`: every tool's issues, one file each, with steps to repeat them, indexed by its `README.md`.
  - `eda/`: example files exported from EDA tools.
  - `logos/` and `screenshots/`: images.
- A crate's own `assets/` folder holds files it needs at runtime, for example `planning/planner_gui_egui/assets/` has
  the GUI's translations. A GUI loads them from the current folder, so it, and its tests, are run from the crate's folder.
- A crate's `docs/dev/` folder holds notes for developers of that crate.
- `gerber/gerber_viewer_egui/examples/` holds example gerber files to view, not Cargo examples.
- `Hacks.md`: what each `HACK:` comment in the code means.

## Tests

- `planning/planner_gui_egui/tests/ui/` is one test binary, `main.rs`, with a module per area of the GUI.

## Tooling

- `support/git/hooks/`: git hooks for the repository, to be installed in each clone. `pre-push` checks formatting.
- `.rustfmt.toml`: formatting, which needs the nightly toolchain, `cargo +nightly fmt`.
- `.claude/settings.json`: Claude Code's settings for the repository.
