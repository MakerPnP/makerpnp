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
  - **Steps to repeat**: numbered steps from a known starting point, using the exact commands, labels and values, each
    followed by **Expected** and **Actual**. Where the core is at fault, give the CLI's steps first: they're the
    quickest to repeat and the easiest to turn into a test. Add the GUI's steps if they differ.
  - **Last repeated**: when the steps were last followed, how (CLI, GUI or a test), on which system and commit, and
    whether the issue was reproduced. Steps that come from reading the code, and haven't been followed, say so.
  - **Notes**: what's known of the cause, and suggested fixes.
  - **Workaround**, if there is one.
- **Keeping the steps up to date.** When a change alters a command, label or workflow that an issue's steps use, update
  the steps in the same change. When an issue is seen again, or its steps are followed, update **Last repeated**, and
  correct the steps if they didn't work as written.
- **Fixing an issue.** Follow the steps to repeat before and after the fix. Add a test that follows them and cites the
  issue's ID, set **Status** to "Fixed on `<date>`, in `<commit>`", name the test in **Checked by**, and keep the steps,
  with **Actual** saying what happened before the fix. The behaviour that was decided becomes a requirement in the
  tool's specification.

### Status

| Status | Meaning |
|---|---|
| **Open** | Not fixed. |
| **Fixed** | Fixed, and checked by a test. |
| **Withdrawn** | Not a problem after all, or no longer possible. The file says why. |

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

## Contributing

The plan is to resolve all of these issues eventually. If you'd like to fix one, pull requests are accepted!
