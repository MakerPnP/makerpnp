# ISSUE-18 — The same document can be opened more than once

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | None yet |
| Found | Before 2026-11-04, in the user's list of known issues |

## Description

The same project, PCB or process can be opened more than once, each in its own tab. Changes saved from one tab are then
overwritten by saving the other.

## Steps to repeat

1. "`Open project`" and open a project.
2. "`Open project`" again and open the same file.

**Expected:** the project's existing tab is shown.

**Actual:** a second tab is opened for the same file.

"`Open PCB`" and "`Open process`" do the same. A project process tab's "`Open file`" doesn't, it shows the open tab
(`PROJPROC-2`).

## Last repeated

Not repeated since it was moved from the user's list of known issues on 2026-11-04. From the code, `open_project_file`,
`open_pcb_file` and `open_process_file` in `ui_app.rs` always add a tab.

## Workaround

Open each document once.
