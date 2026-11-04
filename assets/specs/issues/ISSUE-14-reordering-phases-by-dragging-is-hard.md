# ISSUE-14 — Phases are hard to reorder by dragging them in the project explorer

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | None yet |
| Found | Before 2026-11-04, in the user's list of known issues |

## Description

Phases are hard to reorder by dragging them in the project explorer, as it isn't clear where an item can be dropped.
The dragged items can also be duplicated.

## Steps to repeat

1. Open a project with three phases, and expand them in the project explorer so their load-outs are shown.
2. Drag the first phase below the second.

**Expected:** it's clear where the phase will be dropped, and afterwards the phases are in the new order, each once.

**Actual:** it isn't clear where it can be dropped, and it may not move, or may be shown twice.

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04.

## Notes

The explorer uses `egui_ltreeview`, which also duplicates the dragged items (a `FIXME` in `explorer_tab.rs`).

## Workaround

Collapse the phases before reordering them, or drag the other way, for example drag the last phase up rather than a
middle phase down below it.
