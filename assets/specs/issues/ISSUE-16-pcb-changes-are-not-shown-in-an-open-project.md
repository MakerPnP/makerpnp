# ISSUE-16 — A PCB's changes aren't shown in an open project that uses it

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `PROJPCB` |
| Found | Before 2026-11-04, in the user's list of known issues |

## Description

Changes made to a PCB are not shown in a project that uses it until the PCB is loaded again.

## Steps to repeat

1. Open a project that uses a PCB with one unit, and open the PCB under "`PCBs`" in the project explorer.
2. "`Open PCB`" and open the same PCB file. Change it to two units and "`Save`".
3. Look at the project's PCB tab, and at "`Unit assignments`" in the project explorer.

**Expected:** the project shows two units.

**Actual:** the project still shows one unit.

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04.

## Workaround

Save the project and close it, edit the PCB, then open the project again.
