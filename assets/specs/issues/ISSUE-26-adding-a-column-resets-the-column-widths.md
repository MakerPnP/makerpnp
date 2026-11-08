# ISSUE-26 — Adding a column to a table changes the other columns' widths

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `FORM-15`, `VBGUI-11` |
| Found | 2026-11-08 |

## Description

When a column is added to a table, every column's width goes back to its default, losing the widths the user resized
the columns to. A column is added, for example, when a field is added to a parts file, or to any data source's file,
which the table shows once it reads the file again. Only the new column's width should be its default; the others
should keep theirs.

## Steps to repeat

From the GUI, `planner_gui_egui --trace planner.log -v`, with a parts file, `parts.csv`, in an empty folder:

```csv
"Manufacturer","Mpn"
"RES_MFR1","RES1"
```

1. Open a parts tab, and choose `parts.csv` as its file.

   Logs, in order:
   - `stores_app: Loaded table. kind: Parts, source: '`

2. Drag the right edge of the "`Manufacturer`" column's header, making the column twice as wide.
3. Close the application, and start it again. The parts tab is restored.

   Logs, in order:
   - `stores_app: Loaded table. kind: Parts, source: '`

4. Change `parts.csv`, adding a column after "`Mpn`", and save it:

   ```csv
   "Manufacturer","Mpn","Comment"
   "RES_MFR1","RES1",""
   ```

   Logs, in order:
   - `stores_app: Received event. event: TableSourceChanged { kind: Parts, `
   - `stores_app: Loaded table. kind: Parts, source: '`

**Expected:** after step 3, and after step 4, the "`Manufacturer`" column is as wide as it was resized to. After step
4, the "`Comment`" column is shown, at its default width.

**Actual:** after step 3, the "`Manufacturer`" column is as wide as it was resized to. After step 4, it's back to its
default width, as is every column.

The logs don't say how wide a column is, so they don't tell Expected from Actual: the columns' widths are seen, and a
regression test checks them by the headers' positions, as `tests/ui/variant_builders.rs` does.

## Last repeated

Not yet followed: the steps come from reading the code, on 2026-11-08, after `c6e3686d` and the change that makes a
table's id from its columns (`ui_components::table_columns::table_id`).

## Notes

`egui_deferred_table` keeps each column's width by its place in the table, in egui's memory, which eframe saves
between runs, and gives a column its default width only when the table has fewer widths kept than columns. Before
`table_id`, a column added between others was given the width of the column that had been in its place, and so on
along the row, and the defaults of new columns, or new defaults, were never used. `table_id` makes the table's id from
its columns, so a table with different columns has no widths kept, and starts from the defaults: no column is given
another's width, but none keeps its own.

Suggested fix: keep the widths by the column, such as by its name, rather than by its place: either in
`egui_deferred_table`, which the project maintains a fork of, or in the GUI, giving the table each column's kept width
as its default, and noting the widths the user resizes columns to.

## Workaround

Resize the columns again after adding a column.
