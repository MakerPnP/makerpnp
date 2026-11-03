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
load-out file, and adding the phase's process to those parts. A placement on the other side of the PCB to the phase is left as it
was, as the core only assigns placements on the phase's side, and the tab MUST then say how many were left as they were, until the
selection or the phase changes. If none are on the phase's side, nothing else happens.

**PLACEMENTS-3 — Removing a phase.** *Observed.* "`Remove`" MUST remove each selected placement that's in the chosen phase from that
phase, and the project then has unsaved changes. Any other selected placement MUST be left as it was, so a phase can be removed from a
selection that spans several phases without changing the others. Until 2026-11-03 it removed every selected placement from whichever
phase it was in (`ISSUE-10`). This has to work with the rows that are still selected after "`Apply`": until 2026-11-03 it did nothing,
as it used what the rows held when they were selected.

**PLACEMENTS-4 — A placement's phase cell.** *Observed.* While every phase is pending, a placement's "`Phase`" cell on the Placements
tab MUST be editable (`FORM-11`), offering "`None`" and the phases on the placement's side of the PCB. Choosing a phase MUST assign the
placement to it, and "`None`" MUST remove it from its phase, and the project then has unsaved changes. The results MUST be the same as
the core's `AssignPlacementsToPhase` event. Once any phase isn't pending, the cell MUST NOT be editable.

## Load-outs

**LOADOUT-1 — Assigning a feeder.** *Observed.* Double-clicking a phase's load-out in the project explorer MUST show its parts.
Editing a part's "`Reference`" (`FORM-11`) MUST assign that feeder to that part only. The result MUST be the same as the CLI's
`project assign-feeder-to-load-out-item`.

**LOADOUT-2 — Load-outs are not saved with the project.** *Observed.* A change to a load-out MUST be written to its file as soon as
it is made. The project has no unsaved changes as a result, so "`Save`" stays disabled.

**LOADOUT-3 — Feeders that can't be used.** *Observed.* Clearing a part's "`Reference`" MUST remove its feeder. An invalid reference is
outlined in red while being edited, and finishing the edit MUST NOT change the load-out, with no other message.

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
- `tests/ui/changes.rs` removes a phase from one placement and from all those still selected, and compares the files with the ones the
  core saves (`PLACEMENTS-3`). It also deselects every row, and checks "`Actions`" becomes disabled (`PLACEMENTS-1`).
- `tests/ui/changes.rs` clears a feeder and compares the load-out, and enters an invalid feeder and checks the load-out doesn't change
  (`LOADOUT-3`).
- `tests/ui/changes.rs` moves an ordering back with "`<`" and compares the file, and cancels the dialog and checks the project hasn't
  changed (`ORDERING-1`, `ORDERING-2`).
- `tests/ui/changes.rs` applies `bottom_1` to three top placements and one bottom one, checks the message, then removes `top_1` from
  the same four, and compares the file with the one the core saves (`PLACEMENTS-2`, `PLACEMENTS-3`). It also applies `top_1` to a
  bottom placement alone, checks the message and that the project has no unsaved changes, and that the message goes once the
  selection changes (`PLACEMENTS-2`).
- `tests/ui/phase_states.rs` chooses a bottom placement's phase in its "`Phase`" cell, checking the top phase isn't offered, and
  chooses "`None`" for a top placement, and compares the files with the ones the core saves (`PLACEMENTS-4`).
- `tests/ui/phase_states.rs` starts a phase, and checks "`Apply`" and "`Remove`" are disabled and a "`Phase`" cell can't be edited
  (`PLACEMENTS-1`, `PLACEMENTS-4`).
- Not covered yet: "`Add parts to load-out.`". Selecting another row after rows have been added or removed has not been checked.
