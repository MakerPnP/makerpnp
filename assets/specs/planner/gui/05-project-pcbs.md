# GUI 05 — A project's PCBs

Adding PCBs to a project.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**PROJPCB-1 — Adding a PCB.** *Observed.* "`Add PCB`", on the project's toolbar, MUST open a file dialog. If the user picks
a PCB file, the core MUST add it to the project, and the project then has unsaved changes. If the user cancels, nothing
changes.

**PROJPCB-2 — The PCB's path.** *Observed.* The GUI MUST store the absolute path of the picked file. The CLI can store a
path relative to the project (`project add-pcb --file relative:...`), and its tests do, so a project made with the GUI
stops working if its folder is moved. The code has a `FUTURE` comment considering relative paths for PCBs inside the
project's folder.

## Acceptance criteria

- `tests/ui/workflow.rs` adds a PCB through the file dialog, saves, and compares the file with the one the core saves for
  the same event with an absolute path (`PROJPCB-1`, `PROJPCB-2`).
- Not covered yet: cancelling the dialog, picking a file that isn't a PCB, adding the same PCB twice, and adding a PCB from
  the Overview tab.
