# GUI 07 — Parts

A project's Parts tab, opened from "`Parts`" in the project explorer.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**PARTS-1 — Choosing parts and a process.** *Observed.* Clicking a row's number MUST select that row, and clicking it again
MUST deselect it. Several rows can be selected. "`Process`" MUST offer the project's processes. "`Actions`" MUST be disabled
until at least one row has been selected and a process has been chosen.

**PARTS-2 — Adding or removing a process.** *Observed.* "`Add`" and "`Remove`", in "`Actions`", MUST add the chosen process
to each selected part, or remove it from each. Each change reaches the core immediately, and the project then has unsaved
changes. The result MUST be the same as the CLI's `project assign-process-to-parts`, which selects parts using regular
expressions where the GUI uses exact matches.

## Acceptance criteria

- `tests/ui/workflow.rs` selects one part, adds a process to it, saves, and compares the file with the one the core saves
  for the CLI's event (`PARTS-1`, `PARTS-2`).
- `tests/ui/changes.rs` removes the process, adds one to two parts after deselecting a third, and compares the files with the ones the
  core saves. It also deselects the only row, and checks "`Actions`" becomes disabled (`PARTS-1`, `PARTS-2`).
- Not covered yet: the filter, and editing a part's processes in its cell.
