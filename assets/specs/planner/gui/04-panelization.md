# GUI 04 — Panelization

The PCB's Panelization tab, opened from "`Panelization`" in the PCB explorer. Its values reach the core when Apply is
clicked.

These requirements are drafted from the code and from using it in tests. Those marked *Observed* have not been reviewed.

**PANEL-1 — Panel size and edge rails.** *Observed.* The tab MUST have number fields for the panel's size and the width of
each of its four edge rails. None can be negative. They are named "`Panel size: X`" and "`Edge rails: Top`", for example.

**PANEL-2 — Design configuration.** *Observed.* The tab MUST have a table with one row per design, with number fields for
the X and Y of its placement offset, gerber offset, origin and size. The offsets and origin may be negative, the size may
not. They are named "`design_a: X placement offset`", for example.

**PANEL-3 — A column's header describes its values.** *Confirmed.* Each value in the design configuration table MUST be in
the column whose header names it. This was a bug until 2026-11-02: the placement and gerber offsets were each shown under
the other's header, so a value entered under one was saved as the other.

**PANEL-4 — Unit positions.** *Observed.* The tab MUST have a table with one row per unit, with number fields for its X,
Y and rotation, and the name of its design. X and Y can't be negative, and the rotation is from 0 up to 360 degrees,
wrapping to 0. They are named "`Unit 1: X`", for example. A unit with no design has no fields.

**PANEL-5 — Applying.** *Observed.* "`Apply`" MUST be disabled unless something has changed. Clicking it MUST send the
values to the core, and the PCB then has unsaved changes. The result MUST be the same as giving those values to the CLI
(`pcb configure-panel-sizing`).

## Acceptance criteria

- `tests/ui/workflow.rs` sets every field of `PANEL-1`, `PANEL-2` and the positions of `PANEL-4`, using different values
  for the placement and gerber offsets, applies and saves, and compares the file with the one the core saves for the CLI's
  event (`PANEL-3`, `PANEL-5`).
- Not covered yet: rotation, "`Reset`", assembly orientation, fiducials, the top and bottom selector, the panel preview,
  and the limits on the values.
