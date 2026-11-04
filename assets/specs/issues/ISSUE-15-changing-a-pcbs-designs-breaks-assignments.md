# ISSUE-15 — Changing which design a PCB's units use breaks a project's assignments

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI, likely the core |
| Requirements | `PCBCFG`, `UNITASG` |
| Found | Before 2026-11-04, in the user's list of known issues |

## Description

Changing which design a PCB's units use causes errors in a project that uses the PCB and has design variants assigned
to those units.

## Steps to repeat

1. "`New PCB`" with two units and two designs, `design_a` and `design_b`, with both units using `design_a`. Save it.
2. "`New project`", "`Add PCB`", and assign a variant of `design_a` to unit 1. Save the project and close it.
3. Open the PCB, change unit 1 to use `design_b`, and save it.
4. Open the project.

**Expected:** the project says unit 1's assignment is for a design the unit no longer uses, and lets the user assign it
again.

**Actual:** errors.

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04. The exact errors aren't recorded;
update these steps with them when it's next repeated.

## Workaround

Remove the project's assignments for the PCB before changing it.
