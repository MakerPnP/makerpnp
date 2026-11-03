# GUI 10 — Artifacts and operations

Generating a project's artifacts, and recording the progress of its phases and placements.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**ARTIFACTS-1 — Generating artifacts.** *Observed.* "`Generate artifacts`", on the project's toolbar, MUST write each phase's placements
file and the project's report to the project's folder. The results MUST be the same as the CLI's `project generate-artifacts`.

**OPERATIONS-1 — A phase's tasks.** *Observed.* A phase's tab MUST show each of its operations and their tasks, with each task's status.
A task with actions available MUST offer them in a combo box named after its operation and task, for example
"`load_pcbs: core::load_pcbs`". Choosing one reaches the core immediately, and the project then has unsaved changes. The result MUST be
the same as the CLI's `project record-phase-operation`.

**OPERATIONS-2 — Recording placements.** *Observed.* While its phase allows it, a placement's "`Placed?`" cell on a phase's tab MUST be
editable (`FORM-11`), offering "`Pending`", "`Placed`" and "`Skipped`". The result MUST be the same as the CLI's
`project record-placements-operation`, which selects placements using regular expressions where the GUI makes one exact match for each.

**OPERATIONS-3 — Resetting.** *Observed.* "`Reset operations`", on the project's toolbar, MUST reset the operations of all the project's
phases and placements, with no confirmation, and the project then has unsaved changes. The result MUST be the same as the CLI's
`project reset-operations`.

## Acceptance criteria

- `tests/ui/workflow.rs` chooses package sources, generates artifacts, checks the order of the placements in the top phase's file, and
  compares the project, both phases' placements files and the JSON report with the ones the core writes for the CLI's events
  (`ARTIFACTS-1`).
- `tests/ui/workflow.rs` starts and completes one task and starts another, saves, and compares the file with the one the core saves for the
  CLI's events (`OPERATIONS-1`).
- `tests/ui/workflow.rs` marks three placements as placed, saves, and compares the file (`OPERATIONS-2`). It then resets the operations, saves,
  and compares the file (`OPERATIONS-3`).
- Not covered yet: the operation history file, whose contents have timestamps, the Markdown report, "`Abandon`", "`Skipped`", a phase that
  doesn't allow changes to its placements, and artifacts that can't be generated, for example a missing load-out.
