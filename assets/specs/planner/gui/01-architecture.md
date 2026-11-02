# GUI 01 — Architecture

How the GUI is structured, and how it uses the core. The GUI's gaps are listed by file, so they can be closed one at a
time, and removed from here as they are.

## The core

**GARCH-1 — Headless core.** *Confirmed.* All planning logic and every operation that changes a document MUST be in
the core. The core MUST NOT depend on a UI toolkit, a windowing system or a native dialog.

**GARCH-2 — The core reads and writes document files.** *Confirmed, for now.* The core MUST load and save projects,
PCBs and the files they refer to. The GUI MUST NOT read or write them itself. This may be reviewed once the GUI's and
CLI's behaviour is covered by tests (the `TODO` on `Event::Save` in `planner_app`).

**GARCH-2a — Gerber files for display.** *Confirmed.* The GUI reads and parses gerber files itself, in order to draw
them (`ui_components/gerber_viewer_ui.rs`, `parse_gerber`). This is the only exception to `GARCH-2`.

**GARCH-3 — One core per document.** *Confirmed.* Each open project and each open PCB MUST have its own instance of the
core. The GUI's `Project` and `Pcb` each own one.

**GARCH-4 — Only `update` uses the core.** *Confirmed.* The core MUST only be called while handling a command, never
while drawing a frame.

## Drawing and changing

**GARCH-5 — Drawing is read-only.** *Target.* A component's `ui` MUST NOT change any state, apart from the state allowed
by `GARCH-6`. It reads state, draws it, and sends commands saying what the user did. The signature `ui(&self, ...)` expresses this, but cannot enforce it,
since state held in a `Value` can still be changed through `&self`.

Gaps, where a `ui` method changes state today:

| Where | What it changes |
|---|---|
| `ui_app.rs`, `UiApp::ui` | Runs the start-up steps, and restores documents, in the first frame. |
| `ui_app.rs`, `UiApp::ui` | Takes the file dialog's answer, and clears the picker. |
| `project/mod.rs`, `ui_app/app_tabs/new_project.rs`, `ui_app/app_tabs/new_pcb.rs`, `project/dialogs/add_phase.rs`, `project/dialogs/package_sources.rs`, `dialogs/manage_gerbers.rs` | Takes the file dialog's answer from the picker. |
| `ui_app.rs`, `project/mod.rs`, `pcb/mod.rs` | Removes closed tabs (`cleanup_tabs`), working around a bug in `egui_dock`. |
| `ui_components/gerber_viewer_ui.rs` | Updates the bounding box, and centres the view when asked to. The user's own pan and zoom is allowed by `GARCH-6`. |
| 11 components, for example `pcb/mod.rs` | Sets egui's global text wrapping style (`global_style_mut`), which affects everything drawn after it. |

**GARCH-5a — Controls edit a copy.** *Confirmed.* A control that needs a `&mut` value MUST be given a local copy. If the
copy changes, `ui` sends a command with the new value, and only `update` stores it (for example
`filter/mod.rs`, `Filter::ui`).

**GARCH-6 — State owned by a widget or component.** *Confirmed.* Widgets and components MAY keep and change, while being
drawn, the state of how they themselves are being shown, as is typical for egui controls. Examples are a table's
visible rows, a tree view's expanded, collapsed and selected items, focus and text cursors, the dock layout as tabs are
dragged, and the gerber viewer's pan and zoom.

**GARCH-7 — All other changes are in `update`.** *Confirmed.* Everything else that changes MUST be changed by a
component's `update`, in response to a command.

**GARCH-8 — A component's commands are its whole interface.** *Target.* A component MUST only be changed by the
commands it declares, and MUST report to its parent only by returning an action. It SHOULD NOT need to know about its
parents, or how its commands reach it.

Gaps: a command has to pass through each level between the application and its component, with code at each level to
wrap it and more to unwrap it (`ui_app.rs`, `send_project_command`, `configure_project_component`, and 43 uses of
`configure_mapper`). `UiCommand::LocateComponent` reaches a PCB by searching for it from `handle_command`.

## Data from the core

**GARCH-9 — Views are a cache.** *Target.* Data from the core is a copy. Any event which may change the core's state
MUST make all of that document's copies stale, including when that event fails, since it may have partly changed the
state. Components MUST NOT need to know which views an operation affects. Stale views that are in use MUST be requested
again without the operation having to ask for them.

Gaps: projects do this (`project/data.rs`, `Cached`). PCBs do not, and each PCB operation requests its views itself
(`pcb/mod.rs`).

**GARCH-10 — View state is not document state.** *Confirmed.* What the GUI shows and how, for example open tabs,
selections, filters and pan and zoom, MUST NOT be stored in the core or in document files. The tab layout is saved by
the GUI between runs. The documents themselves MUST NOT be: tabs are restored by loading each document from its file
again.

## Threads

**GARCH-11 — Commands are handled in order.** *Target.* Commands MUST be handled one at a time, in the order they were
sent, on the thread that draws the frames. No component needs a lock, and a frame always sees a finished update.

Gaps: the code is not there yet. Frames are drawn on the UI thread, and commands are handled on a separate thread
(`ui_app.rs`, `with_file_dialogs`). Each `configure_mapper` starts another thread (`sync.rs`, `Slot::start`), so
commands from different components have no defined order. Follow-up commands are sent through a tokio runtime
(`task/`, `runtime/`). Most state is in a `Value`, which is a mutex, and tests have to wait for results as a
consequence.

**GARCH-12 — Short operations.** *Confirmed.* While the core is handling an event, the GUI MAY stop responding, as long
as the operation is short. `AppTabs` stays locked throughout `update`, and every frame needs that lock. Moving the work
to the UI thread (`GARCH-11`) does not change this.

**GARCH-12a — Long operations.** *Confirmed.* An operation known to take a long time MUST NOT be a single event that the
GUI waits for. The core MUST instead have an event to submit it, which returns an identifier, and events that use that
identifier to check its status and retrieve its results, as necessary. No operation is currently known to need this, and
the core has no such events yet. This belongs in the core's specification once there is one.

## Testing

**GARCH-13 — Platform services are replaceable.** *Target.* Anything that needs the user's desktop or operating system,
such as a native file dialog, MUST sit behind an interface, so that tests can supply a fake.

Gaps: `FileDialogs` exists (`file_picker.rs`), and `UiApp::with_file_dialogs` chooses which to use. `dialogs/manage_gerbers.rs`
still always uses the native dialogs, and so do New project and New PCB tabs restored from saved state.

**GARCH-14 — The whole GUI can be tested without a window.** *Confirmed.* Tests MUST run the real application, with its
real core and real files in a temporary folder, without opening a window and without using or changing the user's saved
state (`tests/ui`).

**GARCH-15 — Controls can be found.** *Target.* Every control MUST be findable in the accessibility tree by its role and
its label. A label SHOULD identify a control within the tab or dialog containing it.

Gaps: buttons, tabs, tree items, table text, and form text and number fields have been found so far. A form links a
field's label to its control only if the field returns that control's response, which some number fields do not
(`pcb/tabs/configuration_tab.rs`, its units field), and the unit range's two number fields have no label. Combo boxes are
in the accessibility tree, but no test uses them. `"Refresh"` appears twice when a project is open, so tests will need to
search within one tab. How the gerber viewer's canvas appears there has not been checked.

## Acceptance criteria

- `cargo tree -p planner_app` lists no UI toolkit, windowing or native dialog crate (`GARCH-1`). Checked by hand on
  2026-11-02, not yet automated.
- Tests in `tests/ui` start the whole application with no window, open a project from a real file through a scripted
  file dialog, and see its tab and toolbar (`GARCH-13`, `GARCH-14`, `GARCH-15`).
- `GARCH-5` has no automated test, and cannot have a complete one. A partial one could draw a frame with no input, and
  check that doing so sends no commands and does not change the saved state.
- Once `GARCH-11` is met, a test can run one frame and check the result directly, with no waiting.
