# ISSUE-2 — A file in a subfolder is stored with the system's separator

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `PROJPCB-2`, `PROJPROC-1` |
| Found | 2026-11-02 |

## Description

A PCB or process in a subfolder of the project's folder is stored with a relative path, using the separator of the
system that added it. A project made on Windows stores `sub\panel_a.pcb.json`, which Linux reads as a file name with a
backslash in it, so it doesn't find the PCB.

## Steps to repeat

1. On Windows, create a folder `job`, and a subfolder `job/sub`.
2. "`New PCB`", save it in `job/sub` as `panel_a`.
3. "`New project`" in `job`, then "`Add PCB`" and choose `job/sub/panel_a.pcb.json`.
4. "`Save`", and look at the project file.
5. Copy `job` to Linux, and open the project.

**Expected:** at step 4, the path is stored as `sub/panel_a.pcb.json`, and at step 5 the PCB is loaded.

**Actual:** at step 4, the path is stored as `sub\panel_a.pcb.json` (JSON-escaped), and at step 5 the PCB isn't found.

"`Add process`" stores the path the same way.

## Last repeated

Not yet repeated. The steps are from reading the code (`pcb_file_reference` in `project/mod.rs` strips the project's
folder from the path and keeps the rest as it is).

Its log lines haven't been recorded yet. Record them, with `--trace planner.log -v`, when the steps are next followed.

## Notes

A PCB or process directly in the project's folder has no separator, so isn't affected. Paths given to the CLI with
`relative=` are stored as typed.

## Workaround

Keep PCBs and processes directly in the project's folder, or edit the separators in the project file.
