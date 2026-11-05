# ISSUE-13 — The actions of a phase's tasks weren't translated

| | |
|---|---|
| Status | Fixed on 2026-11-03, in `1a9ba8e3` |
| Tool | Planner: GUI |
| Requirements | `OPERATIONS-1` |
| Found | 2026-11-03 |
| Checked by | `src/i18n/conversions.rs`, `every_task_action_is_translated` |

## Description

The actions a phase's tasks offer, such as "`Start`", "`Complete`" and "`Abandon`", were shown with their names from
the code, in English, whatever the language.

## Steps to repeat

1. Run the GUI in Spanish (`es-ES`).
2. Open a project with a phase, and open the phase's tab.
3. Open a task's combo box.

**Expected:** the actions are in Spanish.

**Actual, before the fix:** the actions were in English.

## Last repeated

2026-11-04, by the test named above: fixed.
