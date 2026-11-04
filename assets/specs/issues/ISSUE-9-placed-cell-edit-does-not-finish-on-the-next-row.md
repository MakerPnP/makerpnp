# ISSUE-9 — Clicking the next row's "Placed?" cell doesn't finish an edit

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `FORM-11`, `OPERATIONS-2` |
| Found | 2026-11-03 |

## Description

While a phase's "`Placed?`" cell is being edited, clicking the same cell of the next row doesn't finish the edit.
Clicking another cell of the row does. So marking several placements row by row takes more clicks than it seems it
should.

## Steps to repeat

1. Open a project with a phase that has at least two placements, and open the phase's tab.
2. Start the phase's tasks, in order, up to and including the one that places components.
3. Click the first placement's "`Placed?`" cell, then click it again to edit it.
4. Click the second placement's "`Placed?`" cell.

**Expected:** the first cell's edit finishes, and the second cell is selected.

**Actual:** the first cell is still being edited.

## Last repeated

Seen failing, and seen working by clicking another cell of the row, in a UI test on 2026-11-03. Not repeated since.

## Notes

The likely cause, from the code, is that the editor's three radio buttons make it taller than its row, so it covers the
rows below.

## Workaround

Click another cell of the row being edited, then the next row's "`Placed?`" cell.
