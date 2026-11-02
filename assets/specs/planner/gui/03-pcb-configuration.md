# GUI 03 — PCB configuration

The PCB's Configuration tab, shown when a PCB has been created or opened. What it does reaches the core when Apply is
clicked. Saving is separate (`DOC-1` covers only the first save, as the PCB is created).

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

## Designs and units

**PCBCFG-1 — Adding a design.** *Observed.* Entering a name in "`Design name`" and clicking "`Add`" MUST add a row to the
designs table. "`Add`" MUST be disabled while the name is empty. A name that is already in the table MUST NOT be added
again. Adding a design MUST NOT clear "`Design name`".

**PCBCFG-2 — Selecting a design.** *Observed.* Clicking a row of the designs table MUST select it, and clicking the
selected row MUST deselect it. At most one design is selected. "`Apply range`", "`Apply all`" and "`Unassign from range`"
MUST be disabled while no design is selected.

**PCBCFG-3 — Assigning a design to a range of units.** *Observed.* "`Unit range: Start`" and "`Unit range: End`" choose
units, counted from 1. "`Apply range`" MUST assign the selected design to each unit in the range, replacing any design
already assigned to it.

**PCBCFG-4 — Applying.** *Observed.* "`Apply`" MUST be disabled unless something has changed and the form is valid, and
every unit needs a design for the form to be valid. Clicking it MUST send the units, the gerber offset, the designs and
the assignments to the core. The PCB then has unsaved changes, and Save is enabled. The result MUST be the same as
creating the PCB with those assignments in the CLI (`pcb create --design`).

## Acceptance criteria

- `tests/ui/workflow.rs` creates a PCB of 4 units, adds 2 designs, assigns them alternately, applies and saves, and
  compares the file with the one the core saves for the CLI's event (`PCBCFG-1`, `PCBCFG-2`, `PCBCFG-3`, `PCBCFG-4`).
- Not covered yet: refusing a duplicate design name, deselecting a design, "`Apply all`", "`Unassign range`", the unit
  mapping table's selection and its buttons, deleting a design, "`Reset`", changing the units of an existing PCB, and the
  gerber offset.
