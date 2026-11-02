# Core 01 — Documents

The core's behaviour when creating documents. Other parts of this area will be added when the slices that use them are
specified.

**DOC-1 — A new document is saved as it is created.** *Confirmed.* Creating a project or a PCB MUST write its file
immediately, and the document MUST then have no unsaved changes. The GUI's Save button is therefore disabled until the
document is next changed.

If the file cannot be written, creating the document MUST report an error.

## Acceptance criteria

- Creating a project through the GUI writes the file, and leaves Save disabled (`tests/ui/workflow.rs`).
- Creating a PCB through the GUI writes the file (`tests/ui/workflow.rs`).
