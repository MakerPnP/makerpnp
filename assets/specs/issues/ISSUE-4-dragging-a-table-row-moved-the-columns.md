# ISSUE-4 — Dragging a table's row moved the columns

| | |
|---|---|
| Status | Fixed on 2026-11-02, in `6879c8c2` |
| Tool | Planner: GUI |
| Requirements | `FORM-9`, `FORM-10` |
| Found | 2026-11-02 |
| Checked by | `tests/ui/tables.rs`, `dragging_a_row_of_a_data_table_moves_the_row_and_not_the_columns` |

## Description

Dragging a row of the Parts, Placements or load-out table moved the table's columns, not the row.

## Steps to repeat

1. Open a project with at least two parts, and open its Parts tab.
2. Drag the first row's number below the second row.

**Expected:** the first part is now the second row, and the columns are in the same order.

**Actual, before the fix:** the rows were in the same order, and the columns had moved.

## Last repeated

2026-11-04, by the test named above: fixed.
