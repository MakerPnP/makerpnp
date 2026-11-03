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

## Fields and rows created on the fly

**FORM-5 — Don't lose what the user entered.** *Confirmed.* The UI MUST always try not to lose user data. Where a value
decides how many fields, rows or other UI elements are shown, reducing that value MUST NOT immediately lose the details
entered in the ones that are no longer shown, so that increasing it again brings them back. This protects against
accidental or unwanted changes, for example to the number of units after making other changes. See `PCBCFG-5`.

## Tables

**FORM-6 — Resize handles.** *Confirmed.* A table whose rows may not all fit MUST have a resize handle, named after the
table for assistive technology and tests, for example "`Unit mapping: Resize`". These are the Designs and Unit mapping
tables of a PCB's Configuration tab, the Fiducials, Design configuration and Unit positions tables of its Panelization tab,
the Phases and PCBs tables of a project's Overview tab, and the Designs and Variant map tables of its Unit assignments tab.

**FORM-7 — Tables are not sized to their rows.** *Confirmed.* A table MUST NOT start tall enough to show all its rows, since
that depends on the other UI elements on the form or tab and on the space available, and is specific to each form or tab.
Rows that don't fit in a table's current height can't be clicked. Dragging its resize handle MUST make the table taller, up
to its maximum height, so that they can be reached.

**FORM-8 — Minimum height.** *Target.* Every table MUST have at least the height of its header and two rows, including a
table with no rows, so that it is obvious that a table is being displayed. A table can't be resized to less than this.

Gaps: when a form or tab has less space than this, tables using `egui_deferred_table` (Parts, Placements and a phase's
load-out) don't keep their minimum size, due to a limitation noted in that library. A table using `egui_deferred_table` with no rows isn't
drawn at all, not even its header, as seen in a test with an empty load-out: that library draws nothing when there are no rows.

**FORM-9 — Moving rows and columns.** *Observed.* In the Parts, Placements and load-out tables, dragging a row's number onto
another row MUST move the row, and dragging a column's header onto another column MUST move the column. This was a bug until
2026-11-02: dragging a row moved columns instead.

**FORM-10 — A moved row's position.** *Observed.* A moved row's position is kept while the number of rows stays the same.
If the number of rows changes, rows MUST return to their original order, so that the table shows each row once.

**FORM-11 — Editing a cell.** *Observed.* In the Parts, Placements and load-out tables, clicking an editable cell MUST select it, and
clicking it again MUST start editing it. Clicking any other cell MUST finish the edit and apply the change. See `ISSUE-9`.

## Acceptance criteria

- `tests/ui/tables.rs` checks a click on unit 4 of the Unit mapping table does nothing, drags the table's handle, and
  checks the same click now selects the unit (`FORM-7`). The UI tests run at 1920x1080.
- `tests/ui/tables.rs` drags a table's handle far upwards, and checks its second row can still be clicked (`FORM-8`). Not covered
  yet: the tables that have no handle, and the other tables with handles, which share the same code.
- `tests/ui/tables.rs` drags a row of the Parts table, and checks the row moved and the columns didn't (`FORM-9`). It then removes
  a part, and checks each row is shown once (`FORM-10`). Not covered yet: the Placements and load-out tables, which have the
  same code, and moving columns.
- `tests/ui/tables.rs` checks the handles exist on a PCB's two tabs and on a project's Overview tab (`FORM-6`). Not covered
  yet: dragging any other handle, and the Unit assignments tab.
- `tests/ui/workflow.rs` edits a load-out's "`Reference`" cell and a phase's "`Placed?`" cells (`FORM-11`). Not covered yet: the cells of the
  Parts table, the Placements table's "`Phase`" cell, an invalid value, and whether there's a way to cancel an edit.
- Each of the fields can be set by answering its dialog (`FORM-2`). `tests/ui/workflow.rs` covers New project, New PCB,
  Package sources and Add phase.
- Clearing a required field stops the form being submitted (`FORM-3`), covered for New project and Add phase.
- Cancelling one of these dialogs leaves the field as it was (`FORM-2`), covered for Add phase. The other fields share the
  same `Picker`, but each handles its answer itself, and they have not been checked.
- `FORM-1` is about appearance, so no test checks it.
- Reducing and then increasing a value keeps what was entered (`FORM-5`), covered for a PCB's units (`PCBCFG-5`). Other
  fields, rows and elements created on the fly have not been checked yet.
