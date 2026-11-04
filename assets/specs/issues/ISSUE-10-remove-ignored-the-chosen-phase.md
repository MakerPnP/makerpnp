# ISSUE-10 — "Remove" on the Placements tab ignored the chosen phase

| | |
|---|---|
| Status | Fixed on 2026-11-03, in `c079e1bf` |
| Tool | Planner: GUI |
| Requirements | `PLACEMENTS-3` |
| Found | 2026-11-03 |
| Checked by | `tests/ui/changes.rs`, `removing_a_phase_leaves_the_placements_of_other_phases_as_they_were` |

## Description

"`Remove`" on the Placements tab ignored the phase chosen in "`Phase`". With `top_1` chosen, it removed a selected
placement from `bottom_1`. "`Actions`" stayed disabled until a phase was chosen, so it looked as if the choice mattered.

## Steps to repeat

1. Open a project with phases `top_1` and `bottom_1`, with some placements on the top in `top_1` and one on the bottom
   in `bottom_1`.
2. On the Placements tab, select a placement in `top_1` and the one in `bottom_1`.
3. Choose `top_1` in "`Phase`", and "`Remove`" in "`Actions`".

**Expected:** the selected placement in `top_1` has no phase, and the one in `bottom_1` is still in `bottom_1`.

**Actual, before the fix:** both placements had no phase.

## Last repeated

2026-11-04, by the test named above: fixed.
