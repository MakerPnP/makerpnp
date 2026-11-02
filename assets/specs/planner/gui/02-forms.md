# GUI 02 — Forms

Controls that are shared by the GUI's forms, in tabs and dialogs.

## Fields chosen using a dialog

**FORM-1 — Layout.** *Confirmed.* A field whose value is chosen using a dialog, for example a file or a directory, MUST
show its value in a read-only text field. A button "`x`" and a button "`...`" MUST be inside the text field's frame, to
the right of the value.

**FORM-2 — Choosing.** *Confirmed.* The "`...`" button MUST open a dialog appropriate to the field, which may be a native
file dialog (`GARCH-13`). If the user picks something, the field MUST show it. If the user cancels, the field MUST NOT
change.

**FORM-3 — Clearing.** *Confirmed.* The "`x`" button MUST clear the value the field is showing. If the field is required,
the form MUST NOT then be submittable (`form-option-error-required`).

**FORM-4 — Names of the buttons.** *Confirmed.* For assistive technology and tests, the "`x`" and "`...`" buttons MUST
be named after their fields, e.g. "`Packages: Clear`" and "`Packages: Choose`" for the field labelled "`Packages`", 
respectively.

Examples of those fields include the directory on the New project and New PCB tabs, the load-out source in the Add phase
dialog, and the packages and package mappings in the Package sources dialog.

## Acceptance criteria

- Each of the fields can be set by answering its dialog (`FORM-2`). `tests/ui/workflow.rs` covers New project, New PCB
  and Package sources. Add phase is not covered yet.
- Clearing a required field stops the form being submitted (`FORM-3`), covered for New project.
- A test that cancels one of these dialogs (`FORM-2`), not covered yet.
- `FORM-1` is about appearance, so no test checks it.
