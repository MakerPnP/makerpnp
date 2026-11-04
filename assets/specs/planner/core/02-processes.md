# Core 02 — Processes

How the user defines the processes that attach components to a PCB, and how a project uses them.

A process is any way of attaching components: placing them with a pick-and-place machine and reflowing solder paste,
placing and soldering them by hand, applying glue and wave soldering, using another machine or a soldering robot, and
so on. The planner doesn't know what each step does. The user defines the steps, and the planner only needs to know the
few kinds of task it has to treat differently (`PROCDEF-2`).

As with a PCB, a process is created and edited in its own file, and a project refers to it.

These requirements were decided by the project's owner, those marked *Observed* are details of how they were done, and
have not been reviewed.

**PROCDEF-1 — A process is defined in its own file.** *Confirmed.* A process MUST be defined in a file of its own, one
process per file, named `<reference>.process.json` when the planner names it. The file holds the process's reference, its
operations, in the order they are done, and its rules. Each operation holds its tasks, in the order they are done.

**PROCDEF-2 — A task has a reference and a kind.** *Confirmed.* Each task MUST have a reference chosen by the user, for
example `apply_glue`, and a kind the core understands:

| Kind | In files | What the core does |
|---|---|---|
| Generic | `generic` | Nothing more than record when the user starts, completes or abandons it. |
| Load PCBs | `load_pcbs` | As generic, and the project's report lists the PCBs to load. *Observed*: added so that the report keeps listing them. |
| Place components | `place_components` | The phase's placements are placed or skipped during it, and it's complete once every one has been (`OPERATIONS-2`). |

**PROCDEF-3 — A process must be valid.** *Observed.* The core MUST refuse a process with two operations with the same
reference, two tasks with the same reference in one operation, or more than one task that places components. A change
that would make a process invalid MUST be refused, and MUST leave the process as it was.

**PROCDEF-4 — Presets.** *Confirmed.* A new process MAY start from a preset, `pnp` or `manual`, which are the processes
the planner used to have built in. Otherwise it has no operations. A new project MUST have no processes.

**PROCDEF-5 — A project has a copy of each process it uses.** *Confirmed.* Adding a process to a project MUST copy its
definition from its file into the project, and store the file's path, relative to the project's folder if it's inside it
(as `PROJPCB-2`). Changing the file afterwards doesn't change the project until its processes are refreshed
(`PROCDEF-6`). *Observed*: adding a file already in the project, or a process whose reference the project already has, is
refused.

**PROCDEF-6 — Refreshing a project's processes.** *Confirmed.* Refreshing MUST read each process's file again, and update
the project's copy. A process that has changed MUST NOT be updated while a phase that uses it has started, and then no
process is updated. *Observed*: if any file can't be read, nothing is updated. A process whose reference has changed is
renamed in the parts and phases that use it.

**PROCDEF-7 — Removing a process from a project.** *Observed.* Removing a process MUST be refused while a phase uses it.
Its file is not deleted, and parts it was assigned to keep it, so that adding it again restores them.

**PROCDEF-8 — Projects from before processes had files.** *Confirmed.* Loading a project whose processes have no files
MUST write each to its own file in the project's folder, refer to it, and leave the project with unsaved changes.
*Observed*: the file is named after the process, `manual.process.json` for example. A file of that name with the same
definition is used as it is, otherwise `manual_2.process.json`, `manual_3.process.json` and so on are tried. The tasks
that used to be built in become tasks of their kind (`core::load_pcbs` is Load PCBs, `core::place_components` is Place
components, the others are Generic), keeping their references.

**PROCDEF-9 — Straddle-mount components.** *Target.* The planner MUST support components that straddle the PCB and are
soldered on both sides, using a process of their own, for example operations and tasks to mount the PCB in a jig, place
the components in the jig, slide the jig into place, solder the top, then solder the bottom (see `../../../pcb-assembly.md`).
Gaps: not yet analysed. A phase is for one side of the PCB (`Phase::pcb_side`), and each placement is on one side, so it
isn't yet known how a phase would cover a component soldered on both.

## Acceptance criteria

- `planning/planning/src/process.rs` checks each way a process can be invalid, and that a refused change leaves it as it
  was (`PROCDEF-3`).
- `planning/planning/src/project.rs` checks a new project has no processes, refuses a duplicate, renames a process in
  the phases and parts that use it, refuses to change one once a phase using it has started, and reads a project from
  before processes had files (`PROCDEF-4` to `PROCDEF-6`, `PROCDEF-8`).
- The CLI's `operation_sequence_1` creates `pnp` from its preset, defines `manual` one change at a time, checks a second
  task that places components is refused, adds both to the project, and refreshes them (`PROCDEF-1` to `PROCDEF-6`).
- The CLI's `legacy_processes` loads a project from before processes had files, with a different `manual.process.json`
  already in its folder, and checks the process is written to `manual_2.process.json` (`PROCDEF-8`).
- Removing a process, and a refresh refused as a phase has started, are covered through the GUI (`tests/ui/processes.rs`).
- Not covered yet: either of them through the CLI.
