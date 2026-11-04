# ISSUE-1 — Creating a document overwrites an existing file

| | |
|---|---|
| Status | Open |
| Tool | Planner: core, so the CLI and the GUI |
| Requirements | `DOC-1` |
| Found | 2026-11-02, widened to processes on 2026-11-04 |

## Description

Creating a project, PCB or process overwrites any existing file of the same name, without warning, as soon as it's
created. The existing document is lost. The GUI's "`Ok`" and the CLI's `create` commands both do it, as both use the
core's events.

## Steps to repeat

From the CLI, in an empty folder:

1. `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`
2. `planner_cli process --process-file pnp.process.json set-reference --reference renamed`
3. Check `pnp.process.json` has `"reference": "renamed"`.
4. `planner_cli process --process-file pnp.process.json create --reference pnp`

**Expected:** step 4 fails with an error saying the file exists, and `pnp.process.json` is unchanged.

**Actual:** step 4 succeeds, and `pnp.process.json` is now a process called `pnp` with no operations.

`project --project job1 create` and `pcb --pcb-file <file> create` do the same.

From the GUI:

1. "`New process`", "`Process reference`" `pnp`, choose a "`Directory`", "`Preset`" `pnp`, "`Ok`".
2. Close the tab, then do step 1 again in the same directory, with "`Preset`" "`None`".

**Expected:** an error saying the file exists. **Actual:** `pnp.process.json` is replaced by an empty process.

## Last repeated

2026-11-04, CLI steps, Windows, after `6f983ed0`: reproduced. The GUI steps are from the code, and haven't been tried.

## Notes

A suggested fix is for the core to refuse to create a document whose file already exists, and for the GUI's form to show
the error. Not decided.

## Workaround

Check the folder before creating a document, and pick another name or folder if the file exists.
