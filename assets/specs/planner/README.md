# Planner — Requirement Specifications

The planner takes a set of PCB designs and plans how they are assembled: which parts are placed in which phase, by
which process, with which feeders. It has one core and two front ends.

These specifications describe what the planner must do, so that it can be changed without guessing what it was
supposed to do, and so that every requirement can be traced to the tests that check it.

## Documents

| Folder | Covers | Crate |
|---|---|---|
| `core/` | Operations, views, files and errors, whichever front end is used | `planning/planner_app` |
| `cli/` | Commands, arguments and output | `planning/planner_cli` |
| `gui/` | Windows, tabs, toolbars, dialogs and controls | `planning/planner_gui_egui` |

A front end's requirements cite the core requirements they depend on, and do not repeat them.

| File | Area | ID prefix |
|---|---|---|
| [core/01-documents.md](core/01-documents.md) | Creating documents | `DOC` |
| [gui/01-architecture.md](gui/01-architecture.md) | How the GUI is structured, and how it uses the core | `GARCH` |
| [gui/02-forms.md](gui/02-forms.md) | Controls shared by the GUI's forms | `FORM` |
| [gui/03-pcb-configuration.md](gui/03-pcb-configuration.md) | A PCB's designs and which units use them | `PCBCFG` |
| [gui/04-panelization.md](gui/04-panelization.md) | A panel's sizes and unit positions | `PANEL` |
| [gui/05-project-pcbs.md](gui/05-project-pcbs.md) | Adding PCBs to a project | `PROJPCB` |
| [known-issues.md](known-issues.md) | Problems known and not yet fixed | `ISSUE` |

## Conventions

- Each requirement has a stable ID such as `GARCH-4`. IDs are never reused. A requirement that is removed is marked
  **Withdrawn** and keeps its ID.
- **MUST** means the requirement is mandatory. **SHOULD** means it is recommended. **MAY** means it is optional.
- Text in `code style` inside quotation marks must be reproduced exactly. It is the `en-US` text.
- Each area ends with **acceptance criteria**: concrete scenarios that automated tests must cover.
- Tests cite the IDs they check, so a requirement's tests can be found by searching for its ID.
- Controls are found in tests by their role and label, as a user would find them, never by position.

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
| **Front end** | A program that uses the core: the GUI or the CLI. |
| **Document** | An open project or PCB. Each is a file. |
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
