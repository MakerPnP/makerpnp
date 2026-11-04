# ISSUE-8 — Adding a phase with a reference that's already used replaces that phase

| | |
|---|---|
| Status | Open |
| Tool | Planner: core, so the CLI and the GUI |
| Requirements | `PHASE-2` |
| Found | 2026-11-03 |

## Description

Adding a phase using a reference that's already used doesn't report an error. It replaces that phase's process and
load-out source, keeps its old PCB side without saying so, and starts the phase's state again, losing its recorded
operations.

## Steps to repeat

From the CLI, in an empty folder:

1. `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`
2. `planner_cli process --process-file manual.process.json create --reference manual --preset manual`
3. `planner_cli project --project job1 create`
4. `planner_cli project --project job1 add-process --file relative=pnp.process.json`, then the same with
   `relative=manual.process.json`
5. `planner_cli project --project job1 create-phase --process pnp --reference top_1 --load-out load_out_1 --pcb-side top`
6. `planner_cli project --project job1 record-phase-operation --phase top_1 --operation load_pcbs --task load_pcbs --action start`,
   then the same with `--action complete`
7. `planner_cli project --project job1 create-phase --process manual --reference top_1 --load-out load_out_2 --pcb-side bottom`

**Expected:** step 7 fails with an error saying `top_1` exists, and the project is unchanged.

**Actual:** step 7 succeeds. `top_1` now has the process `manual` and the load-out `load_out_2`, its side is still
`top`, and its `load_pcbs` task is no longer complete.

From the GUI, "`Add phase`" with a reference that's already used does the same.

## Last repeated

2026-11-04, CLI steps, Windows, after `6f983ed0`: reproduced. Replacing the process and load-out and keeping the side
were also seen through the GUI, in a test, on 2026-11-03.

## Notes

`Project::update_phase` does the replacing. The Add phase form has a `FUTURE` note that it could validate the reference.

## Workaround

Check the project explorer's "`Phases`" before adding a phase, and use a reference that isn't there.
