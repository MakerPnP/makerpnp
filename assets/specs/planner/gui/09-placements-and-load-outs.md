# GUI 09 — Placements and load-outs

Assigning placements to phases, assigning feeders to a load-out's parts, and choosing the order of a phase's placements.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

## Assigning placements to a phase

**PLACEMENTS-1 — Choosing placements and a phase.** *Observed.* On the Placements tab, opened from "`Placements`" in the project
explorer, clicking a row's number MUST select or deselect it, and several rows can be selected. "`Phase`" MUST offer the project's
phases, and is disabled while there are none. "`Actions`" MUST be disabled until at least one row has been selected and a phase
has been chosen. "`Apply`" and "`Remove`", in "`Actions`", MUST be disabled unless every phase is pending.

**PLACEMENTS-2 — Result.** *Observed.* "`Apply`" MUST assign each selected placement to the phase, and the project then has unsaved
changes. The result MUST be the same as the CLI's `project assign-placements-to-phase`, which selects placements using a regular
expression where the GUI makes one exact match for each. This includes the core adding the placements' parts to the phase's
load-out file, and adding the phase's process to those parts.

## Load-outs

**LOADOUT-1 — Assigning a feeder.** *Observed.* Double-clicking a phase's load-out in the project explorer MUST show its parts.
Editing a part's "`Reference`" (`FORM-11`) MUST assign that feeder to that part only. The result MUST be the same as the CLI's
`project assign-feeder-to-load-out-item`.

**LOADOUT-2 — Load-outs are not saved with the project.** *Observed.* A change to a load-out MUST be written to its file as soon as
it is made. The project has no unsaved changes as a result, so "`Save`" stays disabled.

## Placement orderings

**ORDERING-1 — The dialog.** *Observed.* "`Placement orderings.`", on a phase's tab, MUST open a dialog with the available orderings
on its left and the chosen ones on its right. Selecting an ordering enables "`>`", which MUST move it to the chosen ones in the order
given by "`Ascending`" and "`Descending`". The order chosen stays set for the orderings that follow. "`<`" MUST move one back.

**ORDERING-2 — Result.** *Observed.* "`Ok`" MUST set the phase's orderings in the order they were chosen, and the project then has
unsaved changes. The result MUST be the same as the CLI's `project set-placement-ordering`.

## Acceptance criteria

- `tests/ui/workflow.rs` selects three placements, applies a phase, saves, and compares the project and the load-out with the ones the
  core saves for the CLI's event (`PLACEMENTS-1`, `PLACEMENTS-2`).
- `tests/ui/workflow.rs` assigns a feeder to one of two parts, checks "`Save`" stays disabled, and compares the load-out with the one
  the core writes for the CLI's event (`LOADOUT-1`, `LOADOUT-2`).
- `tests/ui/workflow.rs` chooses six orderings, one of them descending, saves, and compares the file with the one the core saves for
  the CLI's event (`ORDERING-1`, `ORDERING-2`).
- Not covered yet: "`Remove`" on the Placements tab and "`<`" in the dialog, the disabled states when a phase is not pending, the
  Placements table's own "`Phase`" cell editor, clearing a feeder, "`Add parts to load-out.`", and cancelling the dialog.
