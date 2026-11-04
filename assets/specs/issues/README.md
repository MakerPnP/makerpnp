# Issues

Problems found in MakerPnP's tools, one file per issue. Each issue has steps to repeat it, which are how a fix is
verified, so they're kept up to date.

## Conventions

- Each issue has an ID such as `ISSUE-7`, and is in a file named after it, `ISSUE-7-<what it is>.md`. IDs are never
  reused, and a file is never deleted.
- Specifications, code and tests cite an issue by its ID.
- Each file has:
  - A table with its **Status**, the **Tool** and the part of it affected (core, CLI or GUI), the **Requirements** it
    concerns, when it was **Found**, and, once fixed, the test it's **Checked by**.
  - **Description**: what's wrong, from the user's point of view.
  - **Steps to repeat**, with **Expected** and **Actual**, see below.
  - **Last repeated**: when the steps were last followed, how (CLI, GUI or a test), on which system and commit, and
    whether the issue was reproduced. Steps that come from reading the code, and haven't been followed, say so.
  - **Notes**: what's known of the cause, and suggested fixes.
  - **Workaround**, if there is one.

### Observing an issue

The planner's logs show what happened, and in what order: each command a GUI component received, each event the core
received, and why the core refused one (`LOG-1` to `LOG-3` in `../planner/core/03-logging.md`). To observe an issue,
follow its steps with the logs written to a file, at debug level:

- CLI: `planner_cli --trace planner.log -v <command>`. Each command adds to the file.
- GUI: `planner_gui_egui --trace planner.log -v`.
- GUI tests: set `PLANNER_GUI_TEST_LOGS` to a folder, and each test's logs are written there.

A log line is quoted by its target and message, leaving out the time, for example
"`planner_app: Event failed. error: ProcessError(ProcessInUse {`". Part of a line is enough, as long as it can't be
mistaken for another.

### Steps to repeat

- Numbered steps from a known starting point, such as an empty folder, using the exact commands, labels and values.
  Where the core is at fault, give the CLI's steps first: they're the quickest to repeat and the easiest to turn into
  a test. Add the GUI's steps if they differ.
- After a step that does something, list the log lines it must produce, in the order they're logged, as "Logs, in
  order:". These show that the step was followed as written: for a click, the component's command (`LOG-3`), and for
  anything sent to the core, the event (`LOG-1`). Other lines may come between them.
- **Expected** and **Actual** each say what the user sees, and the log lines that show it, in order. Name a line that
  must not be logged as such, for example "no `Event failed.`". The log lines are what tell Expected from Actual, so
  give at least one line that Expected has and Actual doesn't, or the other way round.
- Steps that haven't been followed, or whose log lines haven't been recorded, say so in **Last repeated**. Record the
  log lines the next time the steps are followed.

### Keeping the steps up to date

When a change alters a command, label, workflow or log message that an issue's steps use, update the steps in the same
change. When an issue is seen again, or its steps are followed, update **Last repeated**, and correct the steps and log
lines if they didn't match.

### What "fixed" means

An issue is fixed when all of these are true:

1. Its steps to repeat, followed after the fix, give **Expected**: what the user sees, and the expected log lines, in
   order, with none of the lines that only **Actual** has.
2. A test follows the steps and checks the expected log lines in order, as well as the result, such as a file's
   content. Where the core was at fault, it's a CLI test, using `assert_contains_inorder!`. Where the GUI was at fault,
   it's a GUI test, using `wait_for_logs_in_order` and `assert_not_logged`. The test cites the issue's ID.
3. The behaviour that was decided is a requirement in the tool's specification, whose acceptance criteria name the log
   lines, in order.
4. **Status** says "Fixed on `<date>`, in `<commit>`", **Checked by** names the test, and **Actual** keeps what happened
   before the fix, with its log lines.

Issues fixed before 2026-11-04 were checked by tests that don't check the logs (`ISSUE-4`, `ISSUE-10` to `ISSUE-13`).

### Status

| Status | Meaning |
|---|---|
| **Open** | Not fixed. |
| **Fixed** | Fixed, as [What "fixed" means](#what-fixed-means) describes. |
| **Withdrawn** | Not a problem after all, or no longer possible. The file says why. |

### A new issue

```markdown
# ISSUE-<n> — <What's wrong, in a few words>

| | |
|---|---|
| Status | Open |
| Tool | Planner: <core, CLI or GUI> |
| Requirements | <IDs, or "None yet"> |
| Found | <date> |

## Description

<What's wrong, from the user's point of view.>

## Steps to repeat

From the CLI, in an empty folder:

1. `planner_cli --trace planner.log -v <command>`

   Logs, in order:
   - `planner_app: Received event. event: <Event> {`

**Expected:** <what the user sees>. Logs, in order:
- `<target>: <message>`

**Actual:** <what the user sees>. Logs, in order:
- `<target>: <message>`

## Last repeated

<date>, <CLI, GUI or a test>, <system>, after `<commit>`: <reproduced or not>.

## Notes

## Workaround
```

## Issues

| ID | Status | Tool | Issue |
|---|---|---|---|
| [`ISSUE-1`](ISSUE-1-creating-a-document-overwrites-its-file.md) | Open | Planner: core | Creating a document overwrites an existing file |
| [`ISSUE-2`](ISSUE-2-subfolder-paths-use-the-system-separator.md) | Open | Planner: GUI | A file in a subfolder is stored with the system's separator |
| [`ISSUE-3`](ISSUE-3-table-minimum-height-assumes-default-row-height.md) | Open | Planner: GUI | A table's minimum height assumes the default row height |
| [`ISSUE-4`](ISSUE-4-dragging-a-table-row-moved-the-columns.md) | Fixed | Planner: GUI | Dragging a table's row moved the columns |
| [`ISSUE-5`](ISSUE-5-pcb-tabs-number-pcbs-from-0.md) | Open | Planner: GUI | A project's PCB tabs number PCBs from 0 |
| [`ISSUE-6`](ISSUE-6-add-phase-cannot-create-a-load-out.md) | Open | Planner: GUI | The Add phase dialog can't create a new load-out |
| [`ISSUE-7`](ISSUE-7-load-outs-are-not-stored-relative-to-the-project.md) | Open | Planner: core | Load-outs aren't stored relative to the project's folder |
| [`ISSUE-8`](ISSUE-8-adding-a-phase-with-a-used-reference-replaces-it.md) | Open | Planner: core | Adding a phase with a reference that's already used replaces that phase |
| [`ISSUE-9`](ISSUE-9-placed-cell-edit-does-not-finish-on-the-next-row.md) | Open | Planner: GUI | Clicking the next row's "Placed?" cell doesn't finish an edit |
| [`ISSUE-10`](ISSUE-10-remove-ignored-the-chosen-phase.md) | Fixed | Planner: GUI | "Remove" on the Placements tab ignored the chosen phase |
| [`ISSUE-11`](ISSUE-11-apply-silently-skipped-the-other-side.md) | Fixed | Planner: GUI | "Apply" silently skipped placements on the other side of the PCB |
| [`ISSUE-12`](ISSUE-12-placements-could-be-recorded-too-early.md) | Fixed | Planner: GUI | A "Placed?" cell could be edited before its task had started |
| [`ISSUE-13`](ISSUE-13-task-actions-were-not-translated.md) | Fixed | Planner: GUI | The actions of a phase's tasks weren't translated |
| [`ISSUE-14`](ISSUE-14-reordering-phases-by-dragging-is-hard.md) | Open | Planner: GUI | Phases are hard to reorder by dragging them in the project explorer |
| [`ISSUE-15`](ISSUE-15-changing-a-pcbs-designs-breaks-assignments.md) | Open | Planner: GUI | Changing which design a PCB's units use breaks a project's assignments |
| [`ISSUE-16`](ISSUE-16-pcb-changes-are-not-shown-in-an-open-project.md) | Open | Planner: GUI | A PCB's changes aren't shown in an open project that uses it |
| [`ISSUE-17`](ISSUE-17-some-tab-layouts-are-not-restored.md) | Open | Planner: GUI | Some layouts of the GUI's tabs aren't saved and restored correctly |
| [`ISSUE-18`](ISSUE-18-a-document-can-be-opened-twice.md) | Open | Planner: GUI | The same document can be opened more than once |
| [`ISSUE-19`](ISSUE-19-docking-a-table-tab-panics-in-debug.md) | Open | Planner: GUI | Putting back a tab with a data table panics in a debug build |
| [`ISSUE-20`](ISSUE-20-renamed-process-tab-shows-a-spinner.md) | Open | Planner: GUI | A process's tab shows a spinner after a refresh renames the process |
| [`ISSUE-21`](ISSUE-21-cli-help-gives-the-wrong-file-reference-format.md) | Open | Planner: CLI | The CLI's help gives the wrong format for a file reference |
| [`ISSUE-22`](ISSUE-22-cli-absolute-paths-keep-their-prefix.md) | Open | Planner: CLI | A file given to the CLI as `absolute=` keeps the prefix in its path |
| [`ISSUE-23`](ISSUE-23-parts-values-that-look-like-numbers-fail-to-load.md) | Open | Variant Builder: core and CLI | A parts file's values that look like numbers failed to load |

## Contributing

The plan is to resolve all of these issues eventually. If you'd like to fix one, pull requests are accepted!
