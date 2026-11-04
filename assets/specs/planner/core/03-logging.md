# Core 03 — Logging

What the planner logs, so that users, developers and agents can see what happened, in what order, whichever front end
was used. The logs are how an issue is observed (see `../issues/README.md`), and how a fix is verified, so the messages
named in issues, acceptance criteria and tests are part of the planner's behaviour.

The planner logs with the `tracing` crate's macros: `error!`, `warn!`, `info!`, `debug!` and `trace!`. A line in a log
file looks like this, the target being the module that logged it:

```
2026-11-04T09:59:03.757071Z  WARN planner_app: Event failed. error: ProcessError(DuplicateProcessFile { process_file: Relative("pnp.process.json") })
```

Issues and tests quote a line by its target and message, for example
"`planner_app: Event failed. error: ProcessError(DuplicateProcessFile {`", leaving out the time, which always differs.

## What is logged

**LOG-1 — Events.** *Confirmed.* The core MUST log each event it receives, before handling it, at debug level, as
"`planner_app: Received event. event: <event>`", the event in Rust's `Debug` format. So the logs show what each front
end asked the core to do, in order, and the same steps through the CLI and the GUI log the same events.

**LOG-2 — Refused events.** *Observed.* When the core refuses an event, it MUST log why at warn level, as
"`planner_app: Event failed. error: <error>`", after the event's line (`LOG-1`). The error is the one the front end
shows the user.

**LOG-3 — The GUI's commands.** *Confirmed.* Each component's `update` MUST log the command it received, before
handling it, at debug level, as "`<Component> command. command: <command>`", the component's type and the command in
`Debug` format, the target being the component's module. The application's own commands are logged the same way, as
"`planner_gui_egui::ui_commands: UiApp command. command: <command>`".

A command passes from the application down to the component it's for, so a click logs, in order, the application's
command, the command of each component it passes through, and the component's own command, then any event that the
component sends to the core (`LOG-1`). For example, clicking "`Remove process`" on a project's process tab logs, among
others:

```
DEBUG planner_gui_egui::ui_commands: UiApp command. command: TabCommand { ... RemoveClicked ... }
DEBUG planner_gui_egui::project: Project command. command: (ProjectKey(1v1), ProcessTabUiCommand { process: Reference("manual"), command: RemoveClicked })
DEBUG planner_gui_egui::project::tabs::process_tab: ProcessTabUi command. command: RemoveClicked
DEBUG planner_app: Received event. event: RemoveProcess { process_reference: Reference("manual") }
```

A component that only passes a command on to another, such as `TabKind`, needn't log it.

**LOG-4 — Messages.** *Confirmed.* Every message MUST be in the form:

```
<message>.[ <context>[, <context>...]]
```

- `<message>` says what happened, as a short sentence ending in a full stop, for example "`Saved project.`". It MUST
  NOT contain any values: they go in the context.
- Each optional `<context>` is "`<key>: <value>`", the key in `snake_case`, and contexts are separated by "`, `".
- A value that is a string MUST be in single quotes, for example "`path: 'pnp.process.json'`". Other values are
  written in their `Display` or `Debug` format, for example "`error_count: 1`" or "`event: Save`". A value in `Debug`
  format may contain strings in double quotes, as Rust writes them, for example
  "`event: RemoveProcess { process_reference: Reference("manual") }`".

Values are kept out of the message so that a message reads the same whatever the values, and so that searching the logs,
and extracting data from them, is easy. It also avoids plurals that depend on a value.

| Bad | Good |
|---|---|
| "`there were 1 error`" | "`One or more errors occurred. error_count: 1, errors: ["access denied. path: '/p0wn.sh', code: 5"]`" |
| "`Loading PCB from panel_a.pcb.json`" | "`Loading PCB. path: 'panel_a.pcb.json'`" |
| "`PanelTabUi::update, command: Refresh`" | "`PanelTabUi command. command: Refresh`" |

A message MUST NOT name the function or type it's logged from as "`<Type>::<function>`", as the line's target already
names the module. Where a module has more than one component, the component's name begins the message, as in `LOG-3`.

Levels MUST be used as follows:

| Level | For |
|---|---|
| `error` | Something went wrong in the planner itself, which the user can't fix by changing what they asked for. |
| `warn` | A request that was refused or failed, for example an event the core refused (`LOG-2`). |
| `info` | A change to a document, or to a file, that the user would expect to see in a log, for example "`Saved project.`". |
| `debug` | What was asked for and decided: commands (`LOG-3`), events (`LOG-1`), and the choices made handling them. |
| `trace` | Detail too frequent or too large for debug, for example what is done on each frame. |

A message named in an issue, in acceptance criteria or in a test MUST NOT be changed without updating them in the same
change. Searching for the message finds them.

Many messages written before 2026-11-04 don't follow this form yet. A message that doesn't MUST be changed to follow it
when the code that logs it is changed.

**LOG-8 — Once only.** *Confirmed.* The same information MUST NOT be logged twice:

- A command is logged once, by the `update` that receives it (`LOG-3`). A mapper, or a parent that passes a command on,
  MUST NOT log it, and an `update` MUST NOT log again the values its command carries.
- An event, and the reason it was refused, are logged once, by the core (`LOG-1`, `LOG-2`). A front end MUST NOT log
  them again, nor log the error the core returns, except as part of the command that passes the error to a
  component to show it (`LOG-3`).
- A document being loaded, saved or created is logged by the core. A front end MUST NOT log it again.

A log line that adds something the command, event or error doesn't show, for example which tab was opened for a
command, or the action a component returned, isn't a repeat.

## Capturing the logs

**LOG-5 — Log files.** *Confirmed.* The CLI and the GUI MUST both accept `--trace [<file>]`, which writes the logs to
the file, `trace.log` if none is given, adding to it if it exists. `-v` MUST include debug messages, and `-vv` trace
messages. Without `-v`, only info, warn and error messages are written. Both MUST write the same format, so an issue's
log lines are the same whichever front end was used. Without `--trace`, the CLI logs to the console, and the GUI logs to
the console as `RUST_LOG` says, for example `RUST_LOG=debug`. A release build of the GUI on Windows has no console, so
its logs are only kept using `--trace`.

To see the lines that `LOG-1` and `LOG-3` describe, run, for example:

- `planner_cli --trace planner.log -v project --project job1 add-process --file relative=pnp.process.json`
- `planner_gui_egui --trace planner.log -v`

**LOG-6 — Tests.** *Observed.* The tests MUST be able to check what was logged:

- The CLI's tests run the CLI with `--trace` and `-vvv`, read the file, and check it using `assert_contains_inorder!`
  (`common/util/src/assert/mod.rs`).
- The GUI's tests capture each test's logs at debug level and above, in the same format without the time (`Logs`
  in `tests/ui/fixture.rs`). `wait_for_logs_in_order` waits for messages in order, and `assert_not_logged` checks a
  message wasn't logged since. A failing test shows its logs. If the environment variable `PLANNER_GUI_TEST_LOGS` names
  a folder, each test's logs are written there as `<module>.<test name>.log`.

**LOG-7 — Order.** *Confirmed.* Where issues, acceptance criteria or tests name log messages, they MUST give them in the
order they must be logged, and tests MUST check that order. Each message is a line, or part of one, and each is found
on a line after the one before. Other lines may come between them. A message that must
not be logged is named as such, for example "no `Event failed.`".

## Acceptance criteria

- `planner_cli/tests/planner.rs`, `sequence_08_define_process`, adds a second task that places components, and checks,
  in order, the `EditProcess` event, "`Edit process.`", and "`Event failed.`" with `MoreThanOnePlaceComponentsTask`
  (`LOG-1`, `LOG-2`, `LOG-7`).
- `planner_cli/tests/planner.rs`, `sequence_09_add_processes`, adds the same process twice, and checks, in order, the
  `AddProcess` event and "`Event failed.`" with `DuplicateProcessFile` (`LOG-1`, `LOG-2`, `LOG-7`).
- `tests/ui/processes.rs` removes a process, and checks, in order, the project's and the process tab's commands and the
  `RemoveProcess` event, and that no "`Event failed.`" was logged. It removes a process used by a phase, and checks, in
  order, the tab's command, the event, and "`Event failed.`" with `ProcessInUse`. It refreshes a process used by a
  phase that has started, and checks, in order, the tab's command, the `RefreshProcesses` event, and
  "`Event failed.`" with `ProcessInProgress` (`LOG-1` to `LOG-3`, `LOG-7`).
- `tests/ui/processes.rs` removes a process used by a phase, and checks that, after "`Event failed.`", the error is only
  logged again in the `Error` command that passes it to the project to show it (`LOG-8`).
- `tests/ui/fixture.rs` checks messages are found only in the order given (`LOG-7`).
- Not covered yet: a test that `--trace` and `-v` write the GUI's log file (`LOG-5`). It was checked by hand on
  2026-11-04.
