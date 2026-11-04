# Planner — Requirement Specifications

The planner takes a set of PCB designs and plans how they are assembled: which parts are placed in which phase, by
which process, with which feeders. It has one core and two front ends.

These specifications describe what the planner must do, so that it can be changed without guessing what it was
supposed to do, and so that every requirement can be traced to the tests that check it.

## Principles

- **A requirement starts with the user's workflow.** Each requirement begins as a change or improvement to how a user
  does their work. Users then use a front end, the GUI or the CLI, to do what they need to do.
- **The core does the work, for every front end.** The planning is done by the core, not by a front end. The goal is
  that the CLI can do everything the GUI can, through the core. Later there may be other front ends, such as APIs and
  MCP servers, and they should be able to do the same.
- **The front ends drive the core.** What the GUI and the CLI need decides the direction of the core, and which events
  it has.
- **New core events are considered carefully.** Before an event is added to the core, consider how each front end
  would use it: the GUI, the CLI, and later APIs and MCP servers. An event says what the user wants done, not how one
  front end shows it. Events that only one front end uses are kept to a minimum, and the requirement that needs one
  says why.
- **The core is the most important thing to test.** A requirement's behaviour is checked through the core, and the
  CLI's tests, first. A front end's tests check that it sends the core the right events and shows what the core
  returns, for example by comparing the files saved through the GUI with those the core saves for the CLI's events.
  Every change to the GUI updates the GUI's tests it affects, and adds tests for anything new.
- **What happened can be seen in the logs.** The core logs each event it receives, and why it refused one, and each of
  the GUI's components logs each command it receives, so the logs show what the user did and what the core was asked
  to do, in order (`LOG`). Issues and acceptance criteria name the log lines that show a behaviour, in order, and tests
  check them in that order.
- **Issues have steps to repeat.** Problems are recorded in [`../issues/`](../issues/README.md), one file each, with
  steps to repeat them that are kept up to date. They're how a fix is verified.

## Documents

| Folder | Covers | Crate |
|---|---|---|
| `core/` | Operations, views, files and errors, whichever front end is used | `planning/planner_app` |
| `cli/` | Commands, arguments and output | `planning/planner_cli` |
| `gui/` | Windows, tabs, toolbars, dialogs and controls | `planning/planner_gui_egui` |

A front end's requirements cite the core requirements they depend on, and do not repeat them.

Issues are not in this folder, they're in [`../issues/`](../issues/README.md), with the other tools' issues, and use the
ID prefix `ISSUE`.

| File | Area | ID prefix |
|---|---|---|
| [core/01-documents.md](core/01-documents.md) | Creating documents | `DOC` |
| [core/02-processes.md](core/02-processes.md) | Defining processes, and a project's copies of them | `PROCDEF` |
| [core/03-logging.md](core/03-logging.md) | What is logged, and how the logs are captured and checked | `LOG` |
| [gui/01-architecture.md](gui/01-architecture.md) | How the GUI is structured, and how it uses the core | `GARCH` |
| [gui/02-forms.md](gui/02-forms.md) | Controls shared by the GUI's forms | `FORM` |
| [gui/03-pcb-configuration.md](gui/03-pcb-configuration.md) | A PCB's designs and which units use them | `PCBCFG` |
| [gui/04-panelization.md](gui/04-panelization.md) | A panel's sizes and unit positions | `PANEL` |
| [gui/05-project-pcbs.md](gui/05-project-pcbs.md) | Adding PCBs to a project | `PROJPCB` |
| [gui/06-unit-assignments.md](gui/06-unit-assignments.md) | Assigning variants to units, and refreshing from placements files | `UNITASG`, `PLACEREF` |
| [gui/07-parts.md](gui/07-parts.md) | Assigning processes to parts | `PARTS` |
| [gui/08-phases.md](gui/08-phases.md) | Creating a project's phases | `PHASE` |
| [gui/09-placements-and-load-outs.md](gui/09-placements-and-load-outs.md) | Assigning placements and feeders, and ordering placements | `PLACEMENTS`, `LOADOUT`, `ORDERING` |
| [gui/10-artifacts-and-operations.md](gui/10-artifacts-and-operations.md) | Generating artifacts, and recording operations | `ARTIFACTS`, `OPERATIONS` |
| [gui/11-processes.md](gui/11-processes.md) | Creating and editing processes, and a project's processes | `PROC`, `PROJPROC` |

## Conventions

- Each requirement has a stable ID such as `GARCH-4`. IDs are never reused. A requirement that is removed is marked
  **Withdrawn** and keeps its ID.
- **MUST** means the requirement is mandatory. **SHOULD** means it is recommended. **MAY** means it is optional.
- Text in `code style` inside quotation marks must be reproduced exactly. It is the `en-US` text.
- Each area ends with **acceptance criteria**: concrete scenarios that automated tests must cover.
- Tests cite the IDs they check, so a requirement's tests can be found by searching for its ID.
- Controls are found in tests by their role and label, as a user would find them, never by position.
- Acceptance criteria may name log lines, by their target and message, which tests must find in the order given
  (`LOG-7`). Changing such a message means changing the criteria and tests that name it.

### Status

Every requirement has a status, because much of the planner was written before these specifications.

| Status | Meaning |
|---|---|
| **Confirmed** | Decided by the project's owner, and the code does it. |
| **Observed** | The code does this, but nobody has yet decided that it should. It may be an accident. |
| **Target** | Decided by the project's owner, but the code does not fully do it yet. The requirement lists the gaps. |
| **Known bug** | The code does something else, and that is wrong. Its test exists and is marked `#[ignore]`. |

## Glossary

| Term | Meaning |
|---|---|
| **Core** | `planner_app`: all the planning logic, with no user interface. |
| **Front end** | A program that uses the core: the GUI or the CLI, and later APIs and MCP servers. |
| **Process** | A way of attaching components to a PCB, as operations done in order, each with tasks done in order. See `PROCDEF`. |
| **Document** | An open project, PCB or process. Each is a file. |
| **Event** | A request to the core, to change something or to return a view. |
| **View** | A copy of some of the core's data, returned by the core for a front end to show. |
| **Component** | Part of the GUI with a `ui` method, which draws it, and an `update` method, which changes it. |
| **Command** | A message to a component's `update`, saying what happened. Usually sent from its `ui`. |
| **Action** | A component's result from `update`, for its parent to handle. |
| **Frame** | One call of the GUI's `ui` methods. There are many per second while anything is happening. |

## Change log

| Date | Change |
|---|---|
| 2026-11-02 | Initial specification: GUI architecture (`GARCH`). |
| 2026-11-02 | Reviewed `GARCH`: all Observed requirements are now Confirmed. Components, as well as widgets, may keep the state of how they are shown (`GARCH-6`). The GUI may stop responding only for short operations (`GARCH-12`), and long ones must be submitted, checked and collected using an identifier (`GARCH-12a`). |
| 2026-11-02 | Added fields chosen using a dialog, with "`x`" and "`...`" buttons inside the field's frame (`FORM`), replacing "`...`" beside the field on the New project and New PCB tabs and in the Add phase dialog. A new project is now saved as it is created, as a new PCB already was (`DOC-1`). |
| 2026-11-02 | Added PCB configuration (`PCBCFG`), panelization (`PANEL`) and a project's PCBs (`PROJPCB`), drafted from the code and awaiting review. In the design configuration table, the placement and gerber offsets are now under their own headers (`PANEL-3`). |
| 2026-11-02 | The UI must try not to lose what the user entered (`FORM-5`, `PCBCFG-5`). A PCB in the project's folder is now stored with a relative path (`PROJPCB-2`). Added the known issues list. |
| 2026-11-02 | "`Assign selected`" and "`Unassign selected`" no longer change units hidden by reducing "`Units`" (`PCBCFG-6`). |
| 2026-11-02 | Tables that may not fit their rows have named resize handles (`FORM-6`), and dragging one makes its rows reachable (`FORM-7`). UI tests now run at 1920x1080. |
| 2026-11-02 | Tables are not sized to their rows (`FORM-7`). Every table is at least tall enough for its header and two rows (`FORM-8`). |
| 2026-11-02 | Added unit assignments, refreshing from placements files (`UNITASG`, `PLACEREF`) and parts (`PARTS`), drafted from the code and awaiting review. |
| 2026-11-02 | Dragging a row of the Parts, Placements or load-out table now moves the row, not the columns (`FORM-9`, `ISSUE-4`). Rows return to their original order if the number of rows changes (`FORM-10`). |
| 2026-11-03 | Added creating phases (`PHASE`), drafted from the code and awaiting review, and three known issues found while doing so (`ISSUE-6`, `ISSUE-7`, `ISSUE-8`). |
| 2026-11-03 | Added placements, load-outs, placement orderings, artifacts and operations (`PLACEMENTS`, `LOADOUT`, `ORDERING`, `ARTIFACTS`, `OPERATIONS`), and editing a table's cells (`FORM-11`), drafted from the code and awaiting review. Added `ISSUE-9`. The GUI's workflow tests now cover every step of the CLI's `operation_sequence_1`. |
| 2026-11-03 | Added removing a phase from placements (`PLACEMENTS-3`), which did nothing straight after "`Apply`" until now, feeders that can't be used (`LOADOUT-3`) and deleting phases (`PHASE-4`), awaiting review. "`Actions`" on the Parts and Placements tabs is now disabled once nothing is selected (`PARTS-1`, `PLACEMENTS-1`). Added `ISSUE-10` and `ISSUE-11`. |
| 2026-11-03 | "`Remove`" on the Placements tab only removes placements from the chosen phase (`PLACEMENTS-3`, `ISSUE-10`). "`Apply`" says how many placements were left as they were as they're on the other side of the PCB (`PLACEMENTS-2`, `ISSUE-11`). |
| 2026-11-03 | Added a placement's phase cell (`PLACEMENTS-4`), and what can't be changed once a phase isn't pending, or before it can start (`PHASE-4`, `OPERATIONS-1`, `OPERATIONS-2`), awaiting review. Added `ISSUE-12`. |
| 2026-11-03 | Fixed `ISSUE-12`: a "`Placed?`" cell is only editable once the task that places the placements has started, and only offers "`Pending`" once it's complete (`OPERATIONS-2`). |
| 2026-11-03 | Added which actions a phase's tasks offer, abandoning a task, and skipping placements (`OPERATIONS-1`, `OPERATIONS-2`), awaiting review. Added `ISSUE-13`. |
| 2026-11-03 | The actions a phase's tasks offer are translated (`OPERATIONS-1`, `ISSUE-13`). |
| 2026-11-03 | Added adding a phase's parts to its load-out (`LOADOUT-4`), awaiting review, and that an empty table isn't drawn at all (`FORM-8`). |
| 2026-11-03 | An older modified state no longer replaces a newer one, which sometimes left "`Save`" disabled after "`Refresh`" while the project had unsaved changes (`GARCH-11`). |
| 2026-11-03 | Commands are now handled on the thread that draws the frames, in the order they were sent, with no tokio runtime (`GARCH-11`). |
| 2026-11-04 | Moved the planner's entries from the user's list of known issues (`KnownIssues.md`), which no longer exists (`ISSUE-14` to `ISSUE-19`). Three were out of date and are not kept: a task can no longer start before those before it are complete, nor a phase before the phases before it, and placements can no longer be recorded before their task has started (`OPERATIONS-1`, `OPERATIONS-2`). |
| 2026-11-04 | Processes are no longer built into the planner. The user defines each in its own file, from a preset or from nothing, and a project keeps a copy of each process it uses, updated when its processes are refreshed (`PROCDEF`). A task has a reference chosen by the user and a kind (`PROCDEF-2`). A new project has no processes, and an old project's processes are moved to their own files when it's loaded (`PROCDEF-8`). Added the GUI's process documents and a project's processes (`PROC`, `PROJPROC`), awaiting review, and `ISSUE-20` to `ISSUE-22`. A new process is saved as it is created (`DOC-1`). |
| 2026-11-04 | Added GUI tests for the rest of process documents and a project's processes (`PROC`, `PROJPROC`): opening a process, the New process form's validation, moving, deleting and editing operations and tasks, rules, invalid processes, "`Open file`", and the refreshes and removals the core refuses. "`Remove process`" is now named without its icon. |
| 2026-11-04 | Added the principles: requirements start with the user's workflow, the core does the work for every front end, so the CLI can do everything the GUI can, new core events are considered for every front end, and the core is the most important thing to test. Moved the known issues to `../issues/`, one file each, with steps to repeat them, including those already fixed (`ISSUE-4`, `ISSUE-10` to `ISSUE-13`). Followed the steps of `ISSUE-1`, `ISSUE-7`, `ISSUE-8`, `ISSUE-21` and `ISSUE-22` from the CLI, and reproduced each. `ISSUE-7` also affects the CLI, and `ISSUE-18` also affects processes. |
| 2026-11-04 | Added logging (`LOG`): the core logs each event it receives, and why it refused one, and each of the GUI's components logs each command it receives. The GUI accepts `--trace` and `-v`, as the CLI does. The GUI's tests capture each test's logs, and check them in order, as the CLI's do. Issues' steps to repeat now name the log lines that show each step and tell Expected from Actual, and an issue is fixed when a test checks the expected log lines in order. Recorded the log lines of `ISSUE-1`, `ISSUE-7`, `ISSUE-8`, `ISSUE-21` and `ISSUE-22`. |
| 2026-11-04 | Log messages must now be in the form "`<message>.[ <context>[, ...]]`", with values only in the context, as "`<key>: <value>`", and strings in single quotes (`LOG-4`). The same information is logged once only (`LOG-8`): removed 84 log lines that repeated a command, event or error already logged, including each mapper's trace, each front end's trace of the events it sent, the GUI's "`core error:`", and the GUI's document load and save messages. "`pcb::navigate.`" and "`project::navigate.`" are now "`Navigating.`", at debug level. In the GUI's tests, each logged message is now found on a line of its own (`LOG-7`). |
