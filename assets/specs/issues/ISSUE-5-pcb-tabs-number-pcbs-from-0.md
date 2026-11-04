# ISSUE-5 — A project's PCB tabs number PCBs from 0

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `PROJPCB`, `UNITASG` |
| Found | 2026-11-02 |

## Description

A project's PCB and Unit assignments tabs number PCBs from 0, for example "`PCB (0)`" and "`Unit assignments (0)`",
while object paths number them from 1, for example `pcb=1::unit=1`. Other numbers shown to the user count from 1.

## Steps to repeat

1. "`New PCB`", with one unit, and save it.
2. "`New project`", then "`Add PCB`" and choose the PCB.
3. In the project explorer, open the PCB under "`PCBs`", and the PCB under "`Unit assignments`".

**Expected:** the tabs are labelled "`PCB (1)`" and "`Unit assignments (1)`", matching the `pcb=1` of the placements'
object paths.

**Actual:** the tabs are labelled "`PCB (0)`" and "`Unit assignments (0)`".

## Last repeated

Seen in the UI tests on 2026-11-02. Not repeated since.

## Notes

The labels are made from the PCB's index, in `project/tabs/pcb_tab.rs` and `project/tabs/unit_assignments_tab.rs`.

## Workaround

Add 1 to the number in the tab's label.
