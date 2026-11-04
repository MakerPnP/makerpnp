# ISSUE-20 — A process's tab shows a spinner after a refresh renames the process

| | |
|---|---|
| Status | Open |
| Tool | Planner: GUI |
| Requirements | `PROJPROC-2`, `PROCDEF-6` |
| Found | 2026-11-04 |

## Description

If refreshing a project's processes renames one, its tab in the project, which is opened by the process's old
reference, shows a spinner until it's closed.

## Steps to repeat

1. `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`, in the project's folder.
2. Open the project in the GUI, "`Add process`", choose `pnp.process.json`, and "`Save`".
3. Open "`pnp`" under "`Processes`" in the project explorer.
4. `planner_cli process --process-file pnp.process.json set-reference --reference pnp_2`
5. Click "`Refresh processes`" on the process's tab.

**Expected:** the tab shows `pnp_2`, or is closed.

**Actual:** the tab shows a spinner. The project explorer shows `pnp_2`, which opens a new tab.

## Last repeated

Not yet repeated. The steps are from reading the code.

## Workaround

Close the tab, and open the process again from the project explorer.
