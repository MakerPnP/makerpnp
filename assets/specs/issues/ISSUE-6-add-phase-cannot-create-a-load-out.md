# ISSUE-6 — The Add phase dialog can't create a new load-out

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `PHASE-3` |
| Found | 2026-11-03 |

## Description

The Add phase dialog's "`...`" button, for the load-out, opens a dialog that only chooses an existing file. So the user
has to create an empty load-out file some other way first. The CLI's `create-phase` just needs a path, and the core
creates the file if it doesn't exist.

## Steps to repeat

1. Open a project that has a process, and click "`Add phase`".
2. Click "`...`" in "`Load-out source`", go to the project's folder, and type a name that isn't a file, for example
   `load_out_1.csv`.

**Expected:** the name is accepted, and "`Ok`" creates the phase and the load-out file.

**Actual:** the dialog doesn't accept a file that doesn't exist.

## Last repeated

Not yet repeated with the real dialog. The steps are from reading the code. The UI tests answer the dialog themselves,
and deliberately use files that exist.

## Notes

Could be fixed with a save dialog, or a "New" button.

## Workaround

Create an empty load-out file first, or create the phase from the CLI, which creates the file:
`planner_cli project --project job1 create-phase --process pnp --reference top_1 --load-out load_out_1 --pcb-side top`.
Run it from the project's folder, see `ISSUE-7`.
