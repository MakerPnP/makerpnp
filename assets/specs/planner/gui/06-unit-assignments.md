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

**UNITASG-5 — A project locating a design's placements.** *Confirmed.* When a variant builder asks to locate
placements, one or more, of a design (`LocateDesignComponent`), each open project MUST check whether any of its PCBs'
units are assigned any variant of that design, by the design's name, and, for each such PCB, have the PCB locate them,
with the PCB's file, the design's name, and each placement's ref-des, position and side, as the variant builder gave
them: once for each PCB, however many of its units use the design, as the design is the same on each. A project with
no such PCB MUST do nothing. The project's own placements aren't used, so a placement is located even if the project
hasn't been refreshed with it since the variant was built (`UNITASG-7`). *Observed*: each project logs the PCBs it has
locate them, none if it has none, such as
"`Design's PCBs. project: '<folder>/project-job1.mpnp.json', design: 'board1', pcbs: ['<folder>/panel_a.pcb.json']`".

**UNITASG-6 — A PCB locating a design's placements.** *Confirmed.* An open PCB a project has locate placements MUST
show them on each of its gerber viewers of the design, found by its name among the PCB's designs, each position
adjusted by the design's placement and gerber offsets, as `LocateComponent` adjusts a placement's design position, as
`UNITASG-8` says. Its panel's viewer isn't given them, as they aren't of one unit. A PCB that isn't open MUST NOT be
opened, and a design that isn't one of the PCB's does nothing, as `LocateComponent` does. *Observed*: the PCB logs it,
such as "`Locating design components on PCB. pcb: '<folder>/panel_a.pcb.json', design: 'board1', ref_des: ['R1', 'C1']`",
and a design that isn't one of its "`Design not found.`", at debug level.

**UNITASG-8 — Placement markers.** *Confirmed.* Each gerber viewer of a PCB, of a design or of its panel, MUST show a
marker at each placement it's given that's on its side, or every one, if it shows both sides. Its markers MUST be
replaced each time placements are located on the PCB, by a variant builder (`UNITASG-6`) or by a project's placements
(`LocateComponent`), so a viewer given none on its side, or none at all, as it's of another design, or the panel's,
has none, and the markers of a placement located before are never left behind on the other side's viewer. A viewer
given markers MUST pan, without changing its zoom, so the centre of the box around them, the bounding box, is the
centre of its view, and, if any part of the box would then be out of the view, zoom so the box fits, with a margin of
10% of the view's width and height on each side. A single marker's box is a point, so the view only pans. A viewer
with no markers MUST keep its view where it is. *Observed*: each viewer logs its markers, such as
"`Placement markers. mode: Design(0), pcb_side: Some(Top), markers: ['R1']`", a placement located by a project being
named by its object path.

**UNITASG-9 — Showing the side the placements are on.** *Confirmed.* When every placement located on a PCB is on one
side, and the PCB has a viewer of the design for each side, both in the same group of tabs, and the other side's is
the one showing, the side's viewer MUST be shown, made the group's active tab, so that, with the top and bottom
viewers in one group, the user sees the markers without switching tabs. Otherwise, such as with placements on both
sides, the viewers in different groups, or another tab showing in the group, the tabs MUST be left as they are.
*Observed*: the PCB logs it, such as "`Showing design tab. pcb: '<folder>/panel_a.pcb.json', design: 'board1', pcb_side: Top`".

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
- `tests/ui/variant_builders.rs`'s `a_selected_placement_is_marked_on_the_pcbs_its_design_is_on` opens a project
  whose PCB's first unit is assigned `design_a` and `variant_a`, opens the PCB with "`Show PCB`", and its viewers of
  `design_a`, the top's then the bottom's, in one group of tabs, the bottom's showing, creates the variant builder of
  `design_a` and `variant_b`, another variant, clicks an EDA placement's row, R1, on the top, and a variant
  placement's, C1, on the bottom, and checks the logs, in order, for each: the variant builder's
  "`Locate design components.`", with the ref-des, the project's "`Design's PCBs.`", naming the PCB, the PCB's
  "`Locating design components on PCB.`", with the design and ref-des, its "`Showing design tab.`", with the side,
  and each viewer's "`Placement markers.`", the other side's with none, and no "`Design not found.`". It's the
  regression test for `ISSUE-27`, R1's marker left on the top's viewer when C1 was located
  (`UNITASG-5`, `UNITASG-6`, `UNITASG-8`, `UNITASG-9`).
- `a_selected_bom_items_placements_are_marked_on_both_sides` builds the variant, with R1 on the top and R2, the same
  part, on the bottom, clicks the BOM item's "`R1, R2`", and checks the top's viewer marks R1 and the bottom's R2, and
  no "`Showing design tab.`" is logged, as they're on both sides (`UNITASG-8`, `UNITASG-9`).
- `a_placement_of_a_design_no_project_uses_is_not_located` clicks a placement of `design_b`, and checks the project
  logs no PCBs, and no "`Locating design components on PCB.`" is logged (`VBGUI-18`, `UNITASG-5`).
- `ui_components::gerber_viewer_ui`'s tests check a viewer given markers that fit is panned to the centre of their box
  without zooming, a single marker only panned, and markers out of view zoomed to fit with a 10% margin (`UNITASG-8`).
- Not covered yet: a project's `LocateComponent` clearing the other viewers' markers, the panel's viewer, the tabs left
  as they are in different groups, or with another tab showing, a PCB that isn't open, and two projects.
