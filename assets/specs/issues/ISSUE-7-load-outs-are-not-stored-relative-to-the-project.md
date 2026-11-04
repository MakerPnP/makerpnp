# ISSUE-7 — Load-outs aren't stored relative to the project's folder

| | |
|---|---|
| Status | Open: the CLI's part fixed on 2026-11-04, with `STORE-11`; the GUI's part open |
| Tool | Planner: core, the CLI and the GUI |
| Requirements | `PHASE-3`, `STORE-11` |
| Found | 2026-11-03 |

## Description

The GUI stores a phase's load-out with the absolute path that was chosen, so after a project's folder is moved its
phases still refer to the load-outs in the old folder. PCBs and processes avoid this with a relative path (`PROJPCB-2`).

The core treated a relative load-out path in three ways: `build_phase_load_out_item_map` treated it as relative to the
project's folder, the other events that read a load-out as relative to the folder above it, and `ensure_load_out` and
`store_items` as relative to the current directory. So the CLI, run from another folder using `--path`, created the
load-out in the wrong folder. Fixed: the core now reads and writes a relative load-out relative to the project's
folder, in every event, through the stores service (`STORE-11`), and logs the path of the file it creates.

## Steps to repeat

From the CLI, in an empty folder `work`:

1. Make a folder `job`, and in it run
   `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`.
2. Still in `job`, run `planner_cli project --project job1 create`, then
   `planner_cli project --project job1 add-process --file relative=pnp.process.json`.
3. Back in `work`, run
   `planner_cli --trace planner.log -v project --path job --project job1 create-phase --process pnp --reference top_1 --load-out load_out_1 --pcb-side top`.

   Logs, in order:
   - `planner_app: Received event. event: Load { path: "job\project-job1.mpnp.json" }`
   - `planner_app: Received event. event: CreatePhase { process: Reference("pnp"), reference: Reference("top_1"), load_out: File("load_out_1"), pcb_side: Top }`
   - `stores_service: Performing stores operation. operation: ReadRecords { kind: LoadOut, source: File("job\load_out_1") }`
   - `planner_app: Created load-out. source: 'job\load_out_1'`
   - `planning::project: Created phase. reference: 'top_1', process: pnp, load_out: "load_out_1"`
   - `stores_service::csv_file: Storing load-out. source: 'job\load_out_1'`
   - `planner_app: Stored records. source: 'job\load_out_1'`

**Expected:** `job/load_out_1` is created, as the project stores `load_out_1`, which is read from the project's folder.

**Actual:** as expected, since 2026-11-04. Before, `work/load_out_1` was created, and the logs said
`stores::load_out: Created load-out. source: 'load_out_1'`, the source as it was typed, so Expected and Actual
couldn't be told apart from the logs. They now name the file created, `job\load_out_1`.

From the GUI:

1. Open a project that has a process, "`Add phase`", choose a load-out file in the project's folder, "`Ok`", "`Save`".
2. Look at the project file, then move the project's folder and open the project from its new location.

**Expected:** the load-out is stored relative to the project's folder, and is found after the move.

**Actual:** the load-out is stored with its absolute path, so the moved project uses the load-out in the old folder.
The `CreatePhase` event logged at step 1 has `load_out: File("<the absolute path>")`.

## Last repeated

2026-11-04, CLI steps with their logs, Windows, with `STORE-11`: fixed, the logs as above, and `job/load_out_1`
created. `planning/planner_cli/tests/planner.rs`'s `a_relative_load_out_is_created_in_the_projects_folder` checks the
logs in order, and where the file is. The GUI's part not repeated: the stored absolute path is checked by the UI
tests, and the effect of moving the folder is from reading the code.

## Notes

The GUI still needs to store a load-out chosen in the project's folder as a relative path, as the core now reads one
relative to the project's folder.

## Workaround

After moving a project made with the GUI, edit the load-outs' paths in the project file.
