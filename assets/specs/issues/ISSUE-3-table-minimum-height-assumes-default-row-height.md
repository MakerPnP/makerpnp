# ISSUE-3 — A table's minimum height assumes the default row height

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `FORM-8` |
| Found | 2026-11-02 |

## Description

The minimum height of a table, tall enough for its header and two rows (`FORM-8`), assumes a row height of 18 and a
spacing of 3 (`TABLE_ROW_HEIGHT` in `forms/mod.rs`). If the rows are a different height, the minimum is wrong.

## Steps to repeat

The GUI has no setting that changes the row height, so this needs a change to the code:

1. In `UiApp::new` (`ui_app.rs`), make the body text larger, for example set the size of `egui::TextStyle::Body` in
   the context's style to 24.
2. Run the GUI, open a project with at least three parts, and open its Parts tab.
3. Drag the table's resize handle up as far as it goes.

**Expected:** the header and two whole rows are shown.

**Actual:** less than two rows are shown, as the rows are taller than 18.

Zooming with Ctrl and + doesn't show the problem, as it scales the row height and the minimum together.

## Last repeated

Not yet repeated. The steps are from reading the code.

Its log lines haven't been recorded yet. Record them, with `--trace planner.log -v`, when the steps are next followed.

## Notes

The minimum needs to be worked out at runtime from the style in use, likely from the body text size, `interact_size`
and `item_spacing`.

## Workaround

None needed with the default style.
