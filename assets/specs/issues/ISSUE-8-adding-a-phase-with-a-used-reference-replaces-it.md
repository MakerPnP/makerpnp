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

From the CLI, in an empty folder. Each command can be run with `--trace planner.log -v` before `project` or `process`:

1. `planner_cli process --process-file pnp.process.json create --reference pnp --preset pnp`
2. `planner_cli process --process-file manual.process.json create --reference manual --preset manual`
3. `planner_cli project --project job1 create`
4. `planner_cli project --project job1 add-process --file relative=pnp.process.json`, then the same with
   `relative=manual.process.json`
5. `planner_cli --trace planner.log -v project --project job1 create-phase --process pnp --reference top_1 --load-out load_out_1 --pcb-side top`

   Logs, in order:
   - `planner_app: Received event. event: CreatePhase { process: Reference("pnp"), reference: Reference("top_1"), load_out: File("load_out_1"), pcb_side: Top }`
   - `planning::project: Created phase. reference: 'top_1', process: pnp, load_out: "load_out_1"`
6. `planner_cli project --project job1 record-phase-operation --phase top_1 --operation load_pcbs --task load_pcbs --action start`,
   then the same with `--action complete`

   Logs, in order, for the second:
   - `planning::project: Marking task as completed. phase: top_1, operation: load_pcbs, task: load_pcbs`
7. `planner_cli --trace planner.log -v project --project job1 create-phase --process manual --reference top_1 --load-out load_out_2 --pcb-side bottom`

   Logs, in order:
   - `planner_app: Received event. event: CreatePhase { process: Reference("manual"), reference: Reference("top_1"), load_out: File("load_out_2"), pcb_side: Bottom }`

**Expected:** step 7 fails with an error saying `top_1` exists, and the project is unchanged. Logs, in order:
- `planner_app: Event failed. error:`, with an error saying the phase exists
- no `planning::project: Updated phase.`
- no `planner_app: Save project.`

**Actual:** step 7 succeeds. `top_1` now has the process `manual` and the load-out `load_out_2`, its side is still
`top`, and its `load_pcbs` task is no longer complete. Logs, in order:
- `planning::project: Updated phase. old: Phase { reference: Reference("top_1"), process: Reference("pnp"), load_out_source: "load_out_1", pcb_side: Top, placement_orderings: [] }, new: Phase { reference: Reference("top_1"), process: Reference("manual"), load_out_source: "load_out_2", pcb_side: Top,`
- `planner_app: Saved project.`
- no `Event failed.`

From the GUI, "`Add phase`" with a reference that's already used does the same, logging the same `CreatePhase` event and
`Updated phase.` after the Add phase dialog's commands.

## Last repeated

2026-11-04, CLI steps with their logs, Windows, after `8a4ed2ae`, with `LOG-1` and `LOG-2` added: reproduced.
Replacing the process and load-out and keeping the side were also seen through the GUI, in a test, on 2026-11-03.

## Notes

`Project::update_phase` does the replacing, and logs `Updated phase.`. The Add phase form has a `FUTURE` note that it
could validate the reference.

## Workaround

Check the project explorer's "`Phases`" before adding a phase, and use a reference that isn't there.
