# ISSUE-7 — Load-outs aren't stored relative to the project's folder

| | |
|---|---|
| Status | Open |
| Tool | Planner: core, the CLI and the GUI |
| Requirements | `PHASE-3` |
| Found | 2026-11-03 |

## Description

The GUI stores a phase's load-out with the absolute path that was chosen, so after a project's folder is moved its
phases still refer to the load-outs in the old folder. PCBs and processes avoid this with a relative path (`PROJPCB-2`).

The core also treats a relative load-out path in two ways: `build_phase_load_out_item_map` treats it as relative to the
project's folder, while `ensure_load_out` and `store_items` treat it as relative to the current directory. So the CLI,
run from another folder using `--path`, creates the load-out in the wrong folder.

## Steps to repeat

From the CLI, in an empty folder `work`:

1. Make a folder `job`, and in it run
   `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`.
2. Still in `job`, run `planner_cli project --project job1 create`, then
   `planner_cli project --project job1 add-process --file relative=pnp.process.json`.
3. Back in `work`, run
   `planner_cli --trace planner.log -v project --path job --project job1 create-phase --process pnp --reference top_1 --load-out load_out_1 --pcb-side top`.

   Logs, in order:
   - `planner_app: Received event. event: Load { path: "job\\project-job1.mpnp.json" }`
   - `planner_app: Received event. event: CreatePhase { process: Reference("pnp"), reference: Reference("top_1"), load_out: File("load_out_1"), pcb_side: Top }`
   - `stores::load_out: Created load-out. source: 'load_out_1'`
   - `planning::project: Created phase. reference: 'top_1', process: pnp, load_out: "load_out_1"`

**Expected:** `job/load_out_1` is created, as the project stores `load_out_1`, which is read from the project's folder.

**Actual:** `work/load_out_1` is created.

The logs are the same either way, as `Created load-out.` gives the source as it was typed, not the file it created. So
this issue is seen by where the file is, not by the logs. A fix should log the path of the file created, so that
Expected and Actual can be told apart from the logs.

From the GUI:

1. Open a project that has a process, "`Add phase`", choose a load-out file in the project's folder, "`Ok`", "`Save`".
2. Look at the project file, then move the project's folder and open the project from its new location.

**Expected:** the load-out is stored relative to the project's folder, and is found after the move.

**Actual:** the load-out is stored with its absolute path, so the moved project uses the load-out in the old folder.
The `CreatePhase` event logged at step 1 has `load_out: File("<the absolute path>")`.

## Last repeated

2026-11-04, CLI steps with their logs, Windows, after `8a4ed2ae`, with `LOG-1` and `LOG-2` added: reproduced. The
stored absolute path is checked by the UI tests. The effect of moving the folder is from reading the code, and the GUI's
log line is from the event's format, not yet seen.

## Notes

Not just a GUI change: the core needs to use the project's folder for relative load-out paths everywhere.

## Workaround

Run the CLI from the project's folder. After moving a project made with the GUI, edit the load-outs' paths in the
project file.
