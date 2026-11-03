# GUI 10 — Artifacts and operations

Generating a project's artifacts, and recording the progress of its phases and placements.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**ARTIFACTS-1 — Generating artifacts.** *Observed.* "`Generate artifacts`", on the project's toolbar, MUST write each phase's placements
file and the project's report to the project's folder. The results MUST be the same as the CLI's `project generate-artifacts`.

**OPERATIONS-1 — A phase's tasks.** *Observed.* A phase's tab MUST show each of its operations and their tasks, with each task's status.
A task with actions available MUST offer them in a combo box named after its operation and task, for example
"`load_pcbs: core::load_pcbs`". Choosing one reaches the core immediately, and the project then has unsaved changes. The result MUST be
the same as the CLI's `project record-phase-operation`. A phase's first task MUST NOT offer any actions until every phase before it is
complete. A task MUST NOT offer actions until the tasks and operations before it are complete. A pending task offers "`Start`", and a
started one "`Abandon`", along with "`Complete`" if it can be completed this way: the task that places a phase's placements can't, it's
complete once every placement has been placed or skipped. Once a task is complete or abandoned, it MUST NOT offer any actions, and an
abandoned task's operation is abandoned too. The actions' names MUST be translated.
Until 2026-11-03 they were shown in English whatever the language (`ISSUE-13`).

**OPERATIONS-2 — Recording placements.** *Observed.* Once the task that places a phase's placements, for example
`core::place_components`, has been started, a placement's "`Placed?`" cell on the phase's tab MUST be editable (`FORM-11`), offering
"`Pending`", "`Placed`" and "`Skipped`". Once that task is complete, which it is when every placement has been placed or skipped, the cell
MUST only offer "`Pending`", along with the placement's current status. The result MUST be the same as the CLI's
`project record-placements-operation`, which selects placements using regular expressions where the GUI makes one exact match for each.
Otherwise, including while that task is pending, the cell MUST NOT be editable. Until 2026-11-03 it was editable once the tasks before it
were complete, and choosing "`Placed`" or "`Skipped`" did nothing, with no message (`ISSUE-12`). These are the same rules the core uses
when recording placements.

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
- `tests/ui/phase_states.rs` checks the second phase's tasks offer no actions while the first is pending, and that a "`Placed?`" cell
  can't be edited before the phase has started, nor once `load_pcbs` is complete and `core::place_components` is still pending
  (`OPERATIONS-1`, `OPERATIONS-2`).
- `tests/ui/phase_states.rs` marks every placement of the top phase as placed, checks a "`Placed?`" cell then only offers "`Pending`" and
  "`Placed`", resets one, and compares the file with the one the core saves (`OPERATIONS-2`).
- `planning/src/phase.rs` checks `PhaseState::can_modify_placements` for each status of the placements task and each operation.
- `tests/ui/operations.rs` abandons the task that places the top phase's placements, checks it and its operation are abandoned, it
  offers no actions and a "`Placed?`" cell can't be edited, saves, and compares the file with the one the core saves (`OPERATIONS-1`,
  `OPERATIONS-2`).
- `tests/ui/operations.rs` skips a placement, saves, and compares the file with the one the core saves. It also places two placements
  and skips the last, and checks the task offers no actions and the next operation's task does (`OPERATIONS-1`, `OPERATIONS-2`).
- `src/i18n/conversions.rs` checks each task action has a translation in each language (`OPERATIONS-1`).
- Not covered yet: the operation history file, whose contents have timestamps, the Markdown report, and artifacts that can't be generated, for example a missing load-out.
