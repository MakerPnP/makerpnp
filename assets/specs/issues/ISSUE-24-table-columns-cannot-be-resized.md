# ISSUE-24 — A data table's columns can't be resized

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI, every `egui_deferred_table` table |
| Requirements | `VBGUI-11` |
| Found | 2026-11-05 |
| Upstream | `egui_deferred_table` 0.4.0, https://github.com/makerpnp/egui_deferred_table |

## Description

Dragging the edge between two of a data table's column headers doesn't resize the column on its left: the column
header is dragged instead, as if to reorder the columns. Every table made with `egui_deferred_table` is affected, a
design variant's data sources' and the project's alike, though each column is resizable, which is its default.

## Steps to repeat

From the GUI, with a design variant whose part mappings are chosen:

1. `planner_gui_egui --trace planner.log -v`
2. Double-click "`Part mappings: <file>`" in the design variant's "`Explorer`".

   Logs, in order:
   - `planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(DataSource { source: PartMappings, `
   - `stores_app: Loaded table. kind: PartMappings, `
3. Drag the edge between the "`Eda`" and "`Name`" column headers 50 points to the right.

**Expected:** the "`Eda`" column is 50 points wider, and the cursor is a column resize cursor over the edge.

**Actual:** the column's width is unchanged, and the cursor isn't a resize cursor over the edge. Nothing is logged for
the drag, either way.

## Last repeated

2026-11-05, by the GUI test `a_data_sources_columns_can_be_resized` in `planning/planner_gui_egui/tests/ui/
variant_builders.rs`, ignored until this is fixed, on Windows 11, on the commit after `0439385f`: reproduced. Also
reproduced with a table of nothing but `egui_deferred_table`, in a test harness of its own, at every point within 10
points of the edge.

## Notes

The cause, found by logging the library's widgets in a copy of it:

- Each column header has a resize handle, a 6 point wide `ui.interact` over its right edge, made with
  `Sense::click_and_drag()`, and then the header cell itself, made later with
  `ui.allocate_rect(cell_clip_rect, Sense::click_and_drag())`, for the columns to be reordered by dragging. As it's made
  later, the header cell is on top.
- Over the edge, the pointer is in the resize handle, but egui's hit test gives the drag to the header cell, of the
  column on the left or the right, so the resize handle is never hovered or dragged.
- With the header cell made with `Sense::click()`, the same drag resizes the column. That's not a fix, as it stops the
  columns being reordered, but it shows the header cell's drag is what takes it.

A fix, in `egui_deferred_table`: make the resize handle after the header cell, so it's on top, or leave the resize
handles' rects out of the header cell's.

## Workaround

None.
