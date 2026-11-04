# ISSUE-19 — Putting back a tab with a data table panics in a debug build

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | None yet |
| Found | Before 2026-11-04, in the user's list of known issues |
| Reported upstream | https://github.com/Adanos020/egui_dock/issues/278 |

## Description

In a debug build, putting back a tab with a data table, after pulling it out of the window, panics.

## Steps to repeat

1. Run a debug build: `cargo run -p planner_gui_egui`.
2. Open a project that has placements, and open its "`Placements`" tab.
3. Drag the tab out of the window, then drag it back in.

**Expected:** the tab is back in the window.

**Actual:** the GUI panics.

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04.

## Workaround

Use a release build, or keep tabs with data tables in the window.
