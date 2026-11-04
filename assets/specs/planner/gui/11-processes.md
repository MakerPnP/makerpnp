# GUI 11 — Processes

Creating and editing process documents, and a project's processes. What a process is, and how a project uses one, is in
`core/02-processes.md` (`PROCDEF`).

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

## Process documents

**PROC-1 — Creating and opening a process.** *Observed.* "`New process`", on the application's toolbar, MUST open a
"`New process`" tab with "`Process reference`", "`Directory`" and "`Preset`", which offers "`None`" and each preset
(`PROCDEF-4`). "`Ok`" MUST be disabled until the reference is valid and a directory is chosen. "`Ok`" MUST create
`<reference>.process.json` in the directory, which is saved as it is created (`DOC-1`), and replace the tab with the
process's own tab, labelled with its file name. "`Open process`" MUST open an existing process file in a tab of its own.

**PROC-2 — Editing a process.** *Observed.* A process's tab MUST show its path, "`Process reference`", its operations in
order, and its rules. Each operation MUST have, named for assistive technology and tests:

| Control | Name |
|---|---|
| Its reference | "`Operation <n>`", counting from 1 |
| Buttons to move it up and down, and to delete it | "`Operation <n>: Move up`", "`Operation <n>: Move down`", "`Operation <n>: Delete`" |
| Each task's reference, kind and delete button | "`Operation <n>, task <m>`", "`Operation <n>, task <m>: Kind`", "`Operation <n>, task <m>: Delete`" |
| A new task's reference and kind, and the button to add it | "`Operation <n>: New task`", "`Operation <n>: New task: Kind`", "`Operation <n>: Add task`" |

"`New operation`" and "`Add operation`" MUST add an operation at the end. Each rule MUST be a checkbox labelled with the
rule's reference. A kind MUST be shown as "`Generic`", "`Load PCBs`" or "`Place components`" (`PROCDEF-2`).

**PROC-3 — Applying and saving.** *Observed.* Changes MUST NOT change the process until "`Apply`" is clicked. "`Apply`"
MUST be disabled until the form differs from the process and describes a valid one (`PROCDEF-3`). While the form isn't
valid, the tab MUST say why, for example that a process can have at most one task that places components. "`Reset`"
MUST put the form back to the process. "`Save`" MUST save the process's file. As `FORM-5`, the form MUST keep the user's
changes when the process is shown again, unless the process now is what the form describes.

## A project's processes

**PROJPROC-1 — Adding a process to a project.** *Observed.* "`Add process`", on the project's toolbar, MUST open a file
dialog. If the user picks a process file, the core MUST add a copy of it to the project (`PROCDEF-5`), and the project then
has unsaved changes. The file's path MUST be stored as a PCB's is (`PROJPCB-2`). If the user cancels, nothing changes.

**PROJPROC-2 — A project's copy of a process.** *Observed.* A process's tab in a project MUST show the project's copy, which
can't be edited there: its file, its reference, its operations with each task's reference and kind, and its rules. It
MUST have "`Open file`", which opens the process's file as a process document (`PROC-1`), or shows it if it's open,
"`Refresh processes`" and "`Remove process`".

**PROJPROC-3 — Refreshing and removing.** *Observed.* "`Refresh processes`" MUST refresh all the project's processes
(`PROCDEF-6`), as the CLI's `project refresh-processes` does. "`Remove process`" MUST remove the process from the project
(`PROCDEF-7`), and close its tab.

## Acceptance criteria

- `tests/ui/workflow.rs` creates `pnp` from its preset, and compares the file with the one the core saves for the CLI's
  event (`PROC-1`, `DOC-1`).
- `tests/ui/workflow.rs` creates a process with no preset, renames it, adds two operations and three tasks of each kind,
  applies, saves, and compares the file with the `manual` preset. It then adds a second task that places components, and
  checks "`Apply`" is disabled and the tab says why, and that "`Reset`" puts the form back (`PROC-1` to `PROC-3`).
- `tests/ui/workflow.rs` adds `pnp` and `manual` to a project through the file dialog, saves, and compares the file with
  the one the core saves for the CLI's events (`PROJPROC-1`). The workflow tests add them before assigning processes to
  parts, as the CLI's `sequence_09_add_processes` does.
- `tests/ui/workflow.rs` changes a process's file, refreshes from the project's process tab, saves, and compares the file
  with the one the core saves (`PROJPROC-2`, `PROJPROC-3`).
- `src/project/mod.rs` removes a process from its tab, and checks the tab is closed (`PROJPROC-3`).
- `tests/ui/processes.rs` checks "`Ok`" on the New process tab needs a valid reference and a directory, and that
  "`Preset`" offers "`None`" and each preset (`PROC-1`).
- `tests/ui/processes.rs` opens `pnp` using "`Open process`", and checks each operation, task, kind and rule is shown, and
  that nothing can be applied, reset or saved (`PROC-1`, `PROC-2`).
- `tests/ui/processes.rs` moves operations up and down, checks the first can't move up and the last can't move down,
  deletes one, applies, saves, and compares the file with the one the core saves for the same definition. It does the
  same after renaming an operation and a task, changing a task's kind, deleting a task and clearing the rule, and then
  checks the rule can be set again (`PROC-2`, `PROC-3`).
- `tests/ui/processes.rs` checks "`Apply`" is disabled, and the tab says why, for a duplicate operation, an invalid
  operation or task reference, and a duplicate task, and that "`Reset`" puts the form back. It checks "`Add task`" and
  "`Add operation`" need a valid reference (`PROC-2`, `PROC-3`).
- `tests/ui/processes.rs` cancels "`Add process`"'s file dialog and checks nothing changed, and adds a process from
  outside the project's folder and checks its absolute path is stored (`PROJPROC-1`).
- `tests/ui/processes.rs` checks a project's process tab shows its file, reference, operations, tasks and kinds, and
  that "`Open file`" opens the process document, and shows the same tab when clicked again (`PROJPROC-2`).
- `tests/ui/processes.rs` removes a process, saves, compares the file with the one the core saves, and checks its tab is
  closed and its file isn't deleted. It checks the tab's command and the `RemoveProcess` event are logged, in order,
  with no "`Event failed.`" (`LOG-1`, `LOG-3`). It checks a process used by a phase can't be removed, and one used by a
  phase that has started can't be refreshed, that the error is shown, that the tab's command, the event and
  "`Event failed.`" are logged, in order (`LOG-2`), and that the project is unchanged. It checks refreshing unchanged
  processes doesn't change the project (`PROJPROC-3`, `PROCDEF-6`, `PROCDEF-7`).
- Not covered yet: restoring a process's tab when the GUI starts again, and `ISSUE-20`.
