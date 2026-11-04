# Hacks

These hacks can be found in the codebase, and the same pattern is used everywhere the corresponding HACK comment is
found.

## table-resize-hack

```
HACK: table-resize-hack - this hack adds a frame around the table using `egui::Frame`, without the frame there are issues:
      1) the bottom row of the table extends outside the boundary of the `Resize` frame.
      2) the resize handle doesn't work from the bottom right corner due to the scrollbars.
```

example:

```rust
Resize::default()
    // ...
    .show(ui, | ui| {
        egui::Frame::new()        // <-- shouldn't need this frame
            .outer_margin(4.0)    // <-- shouldn't need this margin
            .show(ui, |ui | {
                // ...
                TableBuilder::new(ui)
                    // ...
                    .body( |mut body | {
                    // ...
                    });
            });
    });
```

## tree-view-dir-activate-expand-hack

```
HACK: tree-view-dir-activate-expand-hack - force-expand directories that are 'activated'
```

When double clicking on a directory in an egui_ltreeview the directory state is expanded or collapsed
this hack forces the node to be open.

Use inside the block that handles `Action::Activate`.

example:

```rust
let (_response, actions) = TreeView::new(...)
   // ...

for action in actions {
    match action {
        Action::Activate(activation) => {
            for node_id in activation.selected {
                // ... handle 'activation'

                // HACK: tree-view-dir-activate-expand-hack
                tree_view_state.expand_node(&node_id);
            }

        }
    }
}
```
## core-io-hack

```
HACK: core-io-hack - the <file> is read or written by the core, until the stores service does it.
```

A core does no IO (`STORE-7`): `clippy.toml` in each core's crate, and in `planning`, which the planner's core uses,
denies the methods and types that touch the disk. The IO the core still does is allowed with
`#[allow(clippy::disallowed_methods, clippy::disallowed_types)]` and this comment, so each is easy to find, and each is
a step of the mission that moves it to the stores service (`assets/missions/variant-builder.md`). Remove the hack, and
the `allow`, once the stores service reads or writes the file.

example:

```rust
// HACK: core-io-hack - the operation history is written by the core, until the stores service writes it.
#[allow(clippy::disallowed_methods, clippy::disallowed_types)]
pub fn write(phase_log_path: PathBuf, operation_history: &Vec<OperationHistoryItem>) -> Result<(), Error> {
```
