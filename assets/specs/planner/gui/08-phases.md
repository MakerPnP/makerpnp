# GUI 08 — Phases

Creating a project's phases using the Add phase dialog, which is opened by "`Add phase`" on the project's toolbar, or by
"`Add`" beside the Phases table of its Overview tab.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**PHASE-1 — The dialog's fields.** *Observed.* The dialog MUST have "`Phase reference`", "`Loadout source`", "`PCB Side`" and
"`Process`", which offers the project's processes. "`Ok`" MUST be disabled until all of them have a valid value. "`Loadout source`"
is a field chosen using a dialog (`FORM-1`).

**PHASE-2 — Result.** *Observed.* "`Ok`" MUST close the dialog and create the phase, and the project then has unsaved changes.
The result MUST be the same as the CLI's `project create-phase`. "`Cancel`" MUST close the dialog and not change the project.

**PHASE-3 — The load-out file.** *Observed.* The load-out MUST be stored using the absolute path that was chosen. The core
creates the file if it doesn't exist. In the GUI it always exists, since a file dialog can only choose an existing file, see
`ISSUE-6` and `ISSUE-7`.

Adding a phase whose reference is already used doesn't create a phase, see `ISSUE-8`.

**PHASE-4 — Deleting a phase.** *Observed.* "`Delete`", in a phase's row of the Overview tab's Phases table, is enabled while the phase
is pending. It MUST delete the phase and close its tabs, with no confirmation, and the project then has unsaved changes. Its load-out
file MUST NOT be deleted. The result MUST be the same as the core's `DeletePhase` event.

## Acceptance criteria

- `tests/ui/workflow.rs` adds a phase, and then a second one with a different process and side, saves after each, and compares
  the files with the ones the core saves for the CLI's events (`PHASE-1`, `PHASE-2`).
- `tests/ui/workflow.rs` checks the load-out can be chosen, keeps its value when its file dialog is cancelled, and that clearing
  it disables "`Ok`" (`PHASE-1`, `FORM-2`, `FORM-3`).
- `tests/ui/workflow.rs` fills in the dialog, cancels it, and checks the project has not changed (`PHASE-2`).
- `tests/ui/changes.rs` checks "`Ok`" is disabled without a process or with an invalid or empty reference, shows the reference's error,
  and that "`Add`" on the Overview tab opens the dialog (`PHASE-1`). A missing side is only checked along with a missing process.
- `tests/ui/changes.rs` deletes a phase with its tabs open, and compares the file with the one the core saves (`PHASE-4`).
- Not covered yet: a phase that isn't pending, and a load-out in another folder. Tests start from an existing empty load-out, so the core creating the file is only covered by the CLI's tests.
