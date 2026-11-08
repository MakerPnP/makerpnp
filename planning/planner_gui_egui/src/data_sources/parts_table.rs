//! A table of the parts of a parts data source, with each part's other values, which are edited in their cells
//! (`PARTSRC-7`).

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Key, TextEdit, Ui};
use egui_deferred_table::{
    AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer,
    EditableTableRenderer, EditorState, TableDimensions,
};
use egui_i18n::tr;
use stores_app::{Part, PartsChange, PartsView, Revision};

mod columns {
    pub const MANUFACTURER_COL: usize = 0;
    pub const MPN_COL: usize = 1;

    /// The columns before the parts' other values.
    pub const FIXED_COLUMN_COUNT: usize = 2;
}
use columns::*;

/// The view the stores core sent, shared with every other table showing the same data source, not a copy of it.
struct PartsDataSource {
    view: Arc<PartsView>,
}

impl DeferredTableDataSource for PartsDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.view.parts.len(),
            column_count: FIXED_COLUMN_COUNT + self.view.field_names.len(),
        }
    }
}

struct PartsRenderer {}

impl DeferredTableRenderer<PartsDataSource> for PartsRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &PartsDataSource) {
        let row = &source.view.parts[cell_index.row];

        match cell_index.column {
            MANUFACTURER_COL => ui.label(&row.part.manufacturer),
            MPN_COL => ui.label(&row.part.mpn),
            column => {
                let name = &source.view.field_names[column - FIXED_COLUMN_COUNT];
                ui.label(
                    row.fields
                        .get(name)
                        .map_or("", |value| value.as_str()),
                )
            }
        };
    }
}

/// The part whose other value is edited, the value's name, the value as it was shown when the edit began, and the
/// revision of the parts shown then, so the change is of the parts as the user saw them, and refused if they've changed
/// since (`STORE-15`).
#[derive(Debug, Clone, PartialEq)]
pub struct EditedValue {
    part: Part,
    name: String,
    value: String,
    revision: Revision,
}

/// The change of a part's other value, based on the revision of the parts it was edited in (`PARTSRC-7`).
#[derive(Debug, Clone, PartialEq)]
pub struct PartsEdit {
    pub revision: Revision,
    pub change: PartsChange,
}

impl EditedValue {
    /// The change setting the value to `value`, if it changes it.
    fn change(self, value: String) -> Option<PartsEdit> {
        (value != self.value).then(|| PartsEdit {
            revision: self.revision,
            change: PartsChange::SetFields {
                part: self.part,
                fields: BTreeMap::from([(self.name, value)]),
            },
        })
    }
}

/// The cell `egui_deferred_table` has chosen, which a click again edits, and the cell being edited, with its text.
pub type PartsTableState = EditorState<String, EditedValue>;

/// A parts table's commands, for the tab to handle with [`update_parts_table`] (`PARTSRC-7`).
#[derive(Debug, Clone)]
pub enum PartsTableUiCommand {
    /// The value of the cell being edited was typed.
    CellEdited(CellIndex, String),
    /// The edit was finished, with Enter, so it's written.
    EditSubmitted,
    /// The edit was abandoned, with Escape, leaving the value as it was.
    EditCancelled,
    /// A change to make, an edit finished by clicking elsewhere.
    Change(PartsEdit),
}

/// Handles a parts table's command, changing the cell edited, and returns the change to have the stores core make, if
/// there's one (`PARTSRC-7`).
pub fn update_parts_table(state: &mut PartsTableState, command: PartsTableUiCommand) -> Option<PartsEdit> {
    match command {
        PartsTableUiCommand::CellEdited(cell, text) => {
            if let Some(CellEditState::Editing(editing, current, _)) = &mut state.state
                && *editing == cell
            {
                *current = text;
            }
            None
        }
        PartsTableUiCommand::EditSubmitted => match state.state.take() {
            Some(CellEditState::Editing(cell, text, edited)) => {
                state.state = Some(CellEditState::Pivot(cell));
                edited.change(text)
            }
            other => {
                state.state = other;
                None
            }
        },
        PartsTableUiCommand::EditCancelled => {
            if let Some(CellEditState::Editing(cell, ..)) = &state.state {
                state.state = Some(CellEditState::Pivot(*cell));
            }
            None
        }
        PartsTableUiCommand::Change(edit) => Some(edit),
    }
}

/// Edits a part's other value, sending each change of its text as a command, so the tab changes what's edited.
struct PartsEditor<'a> {
    send: &'a dyn Fn(PartsTableUiCommand),
    /// Whether a change to the data source is being written, so the parts shown aren't yet as it leaves them, and an
    /// edit begun now would be refused.
    changing: bool,
}

impl EditableTableRenderer<PartsDataSource> for PartsEditor<'_> {
    type Value = EditedValue;
    type ItemState = String;

    /// Only a part's other values, as a part is named by its manufacturer and MPN (`STORE-15`).
    fn build_item_state(&self, cell_index: CellIndex, source: &mut PartsDataSource) -> Option<(String, EditedValue)> {
        if self.changing {
            return None;
        }
        let name = cell_index
            .column
            .checked_sub(FIXED_COLUMN_COUNT)
            .and_then(|index| source.view.field_names.get(index))?;
        let item = source.view.parts.get(cell_index.row)?;
        let value = item
            .fields
            .get(name)
            .cloned()
            .unwrap_or_default();
        Some((value.clone(), EditedValue {
            part: item.part.clone(),
            name: name.clone(),
            value,
            revision: source.view.revision.clone(),
        }))
    }

    /// Finished by clicking elsewhere.
    fn on_edit_complete(
        &mut self,
        _cell_index: CellIndex,
        text: String,
        edited: EditedValue,
        _source: &mut PartsDataSource,
    ) {
        if let Some(edit) = edited.change(text) {
            (self.send)(PartsTableUiCommand::Change(edit));
        }
    }

    fn render_cell_editor(
        &self,
        ui: &mut Ui,
        cell_index: &CellIndex,
        text: &mut String,
        _edited: &EditedValue,
        _source: &mut PartsDataSource,
    ) {
        let mut edited = text.clone();
        let response = ui.add(
            TextEdit::singleline(&mut edited)
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY),
        );
        match response.lost_focus() {
            true if ui.input(|input| input.key_pressed(Key::Escape)) => (self.send)(PartsTableUiCommand::EditCancelled),
            true if ui.input(|input| input.key_pressed(Key::Enter)) => (self.send)(PartsTableUiCommand::EditSubmitted),
            true => {}
            false => response.request_focus(),
        }
        if response.changed() {
            (self.send)(PartsTableUiCommand::CellEdited(*cell_index, edited));
        }
    }
}

/// A parts table's editing: its state, kept by the tab across frames, how it sends its commands, and whether a change
/// is being written (`PARTSRC-7`).
pub struct PartsEditing<'a> {
    pub state: &'a mut PartsTableState,
    pub send: &'a dyn Fn(PartsTableUiCommand),
    pub changing: bool,
}

/// Shows the parts, their other values edited in their cells, if `editing` is given (`PARTSRC-7`).
#[profiling::function]
pub fn show_parts_table(ui: &mut Ui, view: Arc<PartsView>, editing: Option<PartsEditing>) {
    let mut column_parameters = vec![
        AxisParameters::default()
            .default_dimension(200.0)
            .name(tr!("table-parts-source-column-manufacturer")),
        AxisParameters::default()
            .default_dimension(200.0)
            .name(tr!("table-parts-source-column-mpn")),
    ];
    column_parameters.extend(view.field_names.iter().map(|name| {
        AxisParameters::default()
            .default_dimension(150.0)
            .name(name.clone())
    }));
    if let Some(last) = column_parameters.pop() {
        column_parameters.push(last.expandable(true));
    }

    let mut source = PartsDataSource {
        view,
    };
    let mut renderer = PartsRenderer {};

    // FUTURE selecting a part selects the corresponding rows of other tables, so the actions will be handled.
    // `egui_deferred_table` takes the space left below what's above it (`ISSUE-25`).
    let table = DeferredTable::new(ui.make_persistent_id("parts_source_table"))
        .min_size((400.0, 400.0).into())
        .column_parameters(&column_parameters);
    match editing {
        Some(editing) => {
            let mut editor = PartsEditor {
                send: editing.send,
                changing: editing.changing,
            };
            let (_response, _actions) = table.show_and_edit(ui, &mut source, &mut renderer, &mut editor, editing.state);
        }
        None => {
            let (_response, _actions) = table.show(ui, &mut source, &mut renderer);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use egui_deferred_table::{CellEditState, CellIndex};
    use stores_app::{Part, PartsChange, Revision};

    use super::{EditedValue, PartsEdit, PartsTableState, PartsTableUiCommand, update_parts_table};

    const CELL: CellIndex = CellIndex {
        row: 1,
        column: 2,
    };

    fn edited() -> EditedValue {
        EditedValue {
            part: Part::new("MFR2".to_string(), "MPN2".to_string()),
            name: "LCSC".to_string(),
            value: "C1".to_string(),
            revision: Revision("r1".to_string()),
        }
    }

    /// The part's `LCSC` value being edited to `text`.
    fn editing(text: &str) -> PartsTableState {
        PartsTableState {
            state: Some(CellEditState::Editing(CELL, text.to_string(), edited())),
        }
    }

    /// `PARTSRC-7`, an edit finished with Enter sets the part's value, keeping the others, based on the revision of the
    /// parts it was edited in, and keeps the cell chosen, so a click on it edits it again.
    #[test]
    fn an_edit_sets_the_parts_value_based_on_the_revision_shown() {
        let mut state = editing("C1");
        update_parts_table(&mut state, PartsTableUiCommand::CellEdited(CELL, "C2".to_string()));

        let edit = update_parts_table(&mut state, PartsTableUiCommand::EditSubmitted);

        assert_eq!(
            edit,
            Some(PartsEdit {
                revision: Revision("r1".to_string()),
                change: PartsChange::SetFields {
                    part: Part::new("MFR2".to_string(), "MPN2".to_string()),
                    fields: BTreeMap::from([("LCSC".to_string(), "C2".to_string())]),
                },
            })
        );
        assert!(matches!(state.state, Some(CellEditState::Pivot(CELL))));
    }

    /// `PARTSRC-7`, an edit leaving the value as it was, or abandoned with Escape, changes nothing, and an edit of
    /// another cell than the one being edited isn't kept.
    #[test]
    fn an_edit_leaving_the_value_or_abandoned_makes_no_change() {
        let mut state = editing("C1");
        assert_eq!(update_parts_table(&mut state, PartsTableUiCommand::EditSubmitted), None);

        let mut state = editing("C1");
        update_parts_table(&mut state, PartsTableUiCommand::CellEdited(CELL, "C2".to_string()));
        assert_eq!(update_parts_table(&mut state, PartsTableUiCommand::EditCancelled), None);
        assert!(matches!(state.state, Some(CellEditState::Pivot(CELL))));
        assert_eq!(update_parts_table(&mut state, PartsTableUiCommand::EditSubmitted), None);

        let mut state = editing("C1");
        let other = CellIndex {
            row: 0,
            column: 2,
        };
        update_parts_table(&mut state, PartsTableUiCommand::CellEdited(other, "C2".to_string()));
        assert_eq!(update_parts_table(&mut state, PartsTableUiCommand::EditSubmitted), None);
    }

    /// `PARTSRC-7`, an edit finished by clicking elsewhere is a change made at once, if it changes the value.
    #[test]
    fn an_edit_finished_elsewhere_is_a_change() {
        assert_eq!(edited().change("C1".to_string()), None);
        let edit = edited().change(String::new()).unwrap();
        let mut state = PartsTableState::default();
        assert_eq!(
            update_parts_table(&mut state, PartsTableUiCommand::Change(edit.clone())),
            Some(edit)
        );
    }
}
