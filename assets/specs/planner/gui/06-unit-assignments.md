# GUI 06 — Unit assignments and placements

A project's Unit assignments tab, which chooses the design variant used by each unit of a PCB, and refreshing the project
from the placements files. The tab is opened by "`Create unit assignment`" on a PCB's tab.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

## Variants and units

**UNITASG-1 — Adding a variant.** *Observed.* "`Design name`" MUST offer the PCB's designs. "`Add`" MUST add the design and
"`Variant name`" to the variants table, and MUST only be enabled once a design is chosen and its placements file exists in
the project's folder, named `<design>_<variant>_placements.csv`. A variant in the table but not assigned to a unit is known
only to the form.

**UNITASG-2 — Selecting a variant.** *Observed.* Clicking a row of the variants table MUST select it, and fill in
"`Design name`" and "`Variant name`" from it. Clicking the selected row MUST deselect it. "`Apply range`", "`Apply all`" and
"`Unassign from range`" MUST be disabled while no variant is selected.

**UNITASG-3 — Assigning a variant.** *Observed.* "`PCB units: Start`" and "`PCB units: End`" choose units, counted from 1.
"`Apply range`" MUST assign the selected variant to each unit in the range that is unassigned or uses the same design, and
skip units using a different design that still exists. Unlike a PCB's Configuration tab, there is no separate Apply: each
change reaches the core immediately, and the project then has unsaved changes.

**UNITASG-4 — Result.** *Observed.* The result MUST be the same as the CLI's `project assign-variant-to-unit`, including
the parts and placements the core adds from the placements file. The project explorer MUST show the assignment.

## Refreshing

**PLACEREF-1 — Refreshing from the placements files.** *Observed.* "`Refresh`", on the project's toolbar, MUST reload the
project's PCBs and its placements files, and the project then has unsaved changes. The result MUST be the same as the CLI's
`project refresh-from-design-variants`, for placements added, removed and changed.

The Issues tab has a "`Refresh`" button of its own, which is why tests select another tab first (`GARCH-15`).

## Acceptance criteria

- `tests/ui/workflow.rs` adds a variant, assigns it to one unit, saves, and compares the file with the one the core saves
  for the CLI's event (`UNITASG-1`, `UNITASG-2`, `UNITASG-3`, `UNITASG-4`).
- `tests/ui/workflow.rs` changes the placements file, refreshes, saves, and compares the file with the one the core saves
  for the CLI's event (`PLACEREF-1`).
- Not covered yet: a missing placements file, skipping a unit using a different design, "`Apply all`", the unassign buttons,
  the Variant map table's selection and its buttons, and dragging this tab's resize handles (`FORM-6`).
