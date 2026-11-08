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

## Locating a design's placement

A variant builder doesn't know which PCB its placements are on: a design is on a PCB's units, and which units use which
design variant is a project's unit assignments. So a variant builder asks every open project to locate a placement
(`VBGUI-18`), and each project decides whether it's on its PCBs. Only the design matters, not the variant: a
placement's ref-des and position come from the design, so they're the same on any PCB with the design, whichever of its
variants a unit is assigned.

**UNITASG-5 — A project locating a design's placement.** *Confirmed.* When a variant builder asks to locate a
placement, of a design (`LocateDesignComponent`), each open project MUST check whether any of its PCBs' units are
assigned any variant of that design, by the design's name, and, for each such PCB, have the PCB locate it, with the
PCB's file, the design's name, and the placement's position and side, as the variant builder gave them: once for each
PCB, however many of its units use the design, as the design is the same on each. A project with no such PCB MUST do
nothing. The project's own placements aren't used, so a placement is located even if the project hasn't been refreshed
with it since the variant was built (`UNITASG-7`). *Observed*: each project logs the PCBs it has locate it, none if it
has none, such as "`Design's PCBs. project: '<folder>/project-job1.mpnp.json', design: 'board1', pcbs: ['<folder>/panel_a.pcb.json']`".

**UNITASG-6 — A PCB locating a design's placement.** *Confirmed.* An open PCB a project has locate a placement MUST centre
each of its gerber viewers showing the design, found by its name among the PCB's designs, on the position, adjusted by
the design's placement and gerber offsets, as `LocateComponent` adjusts a placement's design position. Its panel's
viewer isn't centred, as the placement isn't of one unit. A PCB that isn't open MUST NOT be opened, and a design that
isn't one of the PCB's does nothing, as `LocateComponent` does. *Observed*: the PCB logs it, such as
"`Locating design component on PCB. pcb: '<folder>/panel_a.pcb.json', design: 'board1', pcb_side: Top, design_position: `",
and a design that isn't one of its "`Design not found.`", at debug level. A viewer of one side of the PCB only locates
a placement on that side, as it does for `LocateComponent`.

**UNITASG-7 — A project's design variants kept current.** *Target.* A project's unit assignments, and the placements it
was last refreshed with, may not be of the variant as it was last built, such as a placement added since. The GUI
SHOULD say so, when locating a placement and elsewhere, so that the user refreshes the project. How it's to know isn't
decided.

Gaps: designs are matched by their names, so two designs with the same name, in different folders, are taken for one.

## Acceptance criteria

- `tests/ui/workflow.rs` adds a variant, assigns it to one unit, saves, and compares the file with the one the core saves
  for the CLI's event (`UNITASG-1`, `UNITASG-2`, `UNITASG-3`, `UNITASG-4`).
- `tests/ui/workflow.rs` changes the placements file, refreshes, saves, and compares the file with the one the core saves
  for the CLI's event (`PLACEREF-1`).
- Not covered yet: a missing placements file, skipping a unit using a different design, "`Apply all`", the unassign buttons,
  the Variant map table's selection and its buttons, and dragging this tab's resize handles (`FORM-6`).
- `tests/ui/variant_builders.rs`'s `a_selected_placement_is_located_on_the_pcbs_its_design_is_on` opens a project
  whose PCB's first unit is assigned `design_a` and `variant_a`, opens the PCB with "`Show PCB`", creates the variant
  builder of `design_a` and `variant_b`, another variant, clicks an EDA placement's row, R1, on the top, and a variant
  placement's, C1, on the bottom, and checks the logs, in order, for each: the variant builder's
  "`Locate design component.`", with the ref-des and side, the project's "`Design's PCBs.`", naming the PCB, and the
  PCB's "`Locating design component on PCB.`", with the design and side, and no "`Design not found.`".
  `a_placement_of_a_design_no_project_uses_is_not_located` does the same for `design_b`, and checks the project logs
  no PCBs, and no "`Locating design component on PCB.`" is logged (`VBGUI-18`, `UNITASG-5`, `UNITASG-6`). Not covered
  yet: where a viewer is centred, as the tests can't see the gerbers, a PCB that isn't open, and two projects.
