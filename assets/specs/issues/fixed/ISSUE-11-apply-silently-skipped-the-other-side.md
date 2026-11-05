# ISSUE-11 — "Apply" silently skipped placements on the other side of the PCB

| | |
|---|---|
| Status | Fixed on 2026-11-03, in `c079e1bf` |
| Tool | Planner: GUI |
| Requirements | `PLACEMENTS-2` |
| Found | 2026-11-03 |
| Checked by | `tests/ui/changes.rs`, `applying_a_phase_to_a_placement_on_the_other_side_says_so` and `removing_a_phase_leaves_the_placements_of_other_phases_as_they_were` |

## Description

"`Apply`" on the Placements tab left any selected placement on the other side of the PCB to the phase as it was, with no
message. If all of them were on the other side, nothing at all appeared to happen.

## Steps to repeat

1. Open a project with a phase `bottom_1` for the bottom of the PCB, and placements on the top.
2. On the Placements tab, select a placement on the top.
3. Choose `bottom_1` in "`Phase`", and "`Apply`" in "`Actions`".

**Expected:** the tab says "1 placement was not changed, it's on the other side of the PCB to the phase."

**Actual, before the fix:** nothing appeared to happen.

## Last repeated

2026-11-04, by the tests named above: fixed.

## Notes

The core leaves them out in `assign_placements_to_phase`, which the CLI's patterns rely on, so the message comes from
the GUI.
