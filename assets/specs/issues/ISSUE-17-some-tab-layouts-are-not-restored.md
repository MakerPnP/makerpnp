# ISSUE-17 — Some layouts of the GUI's tabs aren't saved and restored correctly

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | None yet |
| Found | Before 2026-11-04, in the user's list of known issues |

## Description

Some layouts of the GUI's tabs are not saved and restored correctly when the GUI is started again.

## Steps to repeat

No reliable steps are known yet. To look for them:

1. Open a project and several of its tabs.
2. Split the window by dragging tabs to its edges, and pull one tab out of the window.
3. Close the GUI and start it again.

**Expected:** the tabs are where they were.

**Actual:** for some layouts, the tabs are not where they were. Record the layout here when one is found.

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04.

Its log lines haven't been recorded yet. Record them, with `--trace planner.log -v`, when the steps are next followed.

## Notes

The cause is in `egui_dock`.

## Workaround

Close the GUI and delete `app.ron`. On Windows it's `%APPDATA%\MakerPnP - Planner\data\app.ron`.
