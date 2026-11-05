# ISSUE-12 — A "Placed?" cell could be edited before its task had started

| | |
|---|---|
| Status | Fixed on 2026-11-03, in `af2a46cf` |
| Tool | Planner: GUI |
| Requirements | `OPERATIONS-2` |
| Found | 2026-11-03 |
| Checked by | `tests/ui/phase_states.rs`, `placements_cannot_be_recorded_before_their_phase_allows_it` |

## Description

A phase's "`Placed?`" cells could be edited before the task that places components had started, and choosing
"`Placed`" or "`Skipped`" then did nothing, with no message. The core refuses these changes, so the GUI shouldn't offer
them.

## Steps to repeat

1. Open a project with a phase using `pnp`, with placements, and open the phase's tab.
2. Click a placement's "`Placed?`" cell.
3. Start and complete `load_pcbs`, then click the cell again.

**Expected:** at steps 2 and 3 the cell can't be edited. Once `place_components` has started it can, and it only offers
"`Pending`" once that task is complete.

**Actual, before the fix:** the cell could be edited, and choosing "`Placed`" did nothing.

## Last repeated

2026-11-04, by the test named above: fixed.
