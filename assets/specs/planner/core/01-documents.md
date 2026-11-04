# Core 01 — Documents

The core's behaviour when creating documents. Other parts of this area will be added when the slices that use them are
specified.

**DOC-1 — A new document is saved as it is created.** *Confirmed.* Creating a project, a PCB or a process MUST write its
file immediately, and the document MUST then have no unsaved changes. The GUI's Save button is therefore disabled until
the document is next changed.

If the file cannot be written, creating the document MUST report an error. A project whose file can't be written MUST
NOT be created, and the project open before it, if any, MUST be kept. The file is written by the stores service
(`STORE-10`), and the project is created once it is.

## Acceptance criteria

- Creating a project through the GUI writes the file, and leaves Save disabled, and
  "`planner_app: Received event. event: CreateProject {`",
  "`stores_service: Performing stores operation. operation: WriteDocument { path: `",
  "`planner_app: Received event. event: ProjectWritten {`" and "`planner_app: Created project successfully.`" are
  logged, in order (`tests/ui/workflow.rs`, `STORE-10`).
- `planning/planner_app/tests/documents.rs` checks a project whose file can't be written isn't created, and that the
  project before it is kept (`STORE-10`).
- Creating a PCB through the GUI writes the file (`tests/ui/workflow.rs`).
- Creating a process through the GUI writes the file, and leaves Save disabled (`tests/ui/workflow.rs`).
