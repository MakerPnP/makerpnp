# GUI 05 — A project's PCBs

Adding PCBs to a project.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**PROJPCB-1 — Adding a PCB.** *Observed.* "`Add PCB`", on the project's toolbar, MUST open a file dialog. If the user picks
a PCB file, the core MUST add it to the project, and the project then has unsaved changes. If the user cancels, nothing
changes.

**PROJPCB-2 — The PCB's path.** *Confirmed.* If the picked file is inside the project's folder, or a subfolder of it, the
GUI MUST store its path relative to that folder, as the CLI can (`project add-pcb --file relative:...`), so that the
project keeps working if the folder is moved. Otherwise the GUI MUST store the absolute path. A relative path is always
relative to the project's folder, not to the project's file. See `ISSUE-2`.

## Acceptance criteria

- `tests/ui/workflow.rs` adds a PCB in the project's folder through the file dialog, saves, and compares the file with the
  one the core saves for the CLI's event, with a relative path (`PROJPCB-1`, `PROJPCB-2`).
- `tests/ui/workflow.rs` moves the project's folder, and opens the project from its new location (`PROJPCB-2`).
- `tests/ui/workflow.rs` adds a PCB from outside the project's folder, and checks that its absolute path is stored
  (`PROJPCB-2`).
- Not covered yet: cancelling the dialog, picking a file that isn't a PCB, adding the same PCB twice, and adding a PCB from
  the Overview tab.
