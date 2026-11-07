//! A table of a data source's records, as the stores core shows any data source other than parts (`STORE-17`), each
//! value shown as what it is, a pattern by its kind (`VBGUI-11`), and, if its kind can be changed, edited in its cells,
//! and its rows added, removed and moved (`VBGUI-13`).

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Button, Color32, ComboBox, Key, TextEdit, Ui, Visuals};
use egui_deferred_table::{
    Action, AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer,
    EditableTableRenderer, EditorState, TableDimensions,
};
use egui_i18n::tr;
use stores_app::{
    Cell, ColumnValue, EdaTool, FieldPattern, Part, PartMappingsViewItem, Pattern, RowKey, RowOrdering, TableChange,
    TableRow, TableView,
};

/// The view the stores core sent, shared with every other table showing the same data source, not a copy of it.
struct TableDataSource {
    view: Arc<TableView>,
}

/// What a cell is being edited to, as it's typed, or chosen (`VBGUI-13`).
#[derive(Debug, Clone, PartialEq)]
pub enum CellEdit {
    /// A pattern, parsed as it's typed, so it's coloured by what it is.
    Pattern(Pattern),
    /// A manufacturer or an MPN.
    Text(String),
    Eda(EdaTool),
}

/// The row a cell's edit is of, as it was shown when the edit began, and the cell's column, so the change is of the row
/// as the user saw it (`STORE-19`).
#[derive(Debug, Clone)]
pub struct EditedRow {
    key: RowKey,
    row: TableRow,
    column: ColumnValue,
}

impl EditedRow {
    /// The change of the row, as it was, to the row with the edit, if the edit changes it.
    fn change(self, edit: CellEdit) -> Option<TableChange> {
        let new = with_edit(&self.row, &self.column, edit);
        (new != self.row).then(|| TableChange::Update {
            key: self.key,
            old: self.row,
            new,
        })
    }
}

/// The cell `egui_deferred_table` has chosen, which a click again edits, and the cell being edited.
pub type TableEditorState = EditorState<CellEdit, EditedRow>;

/// A changeable table's state, kept by the tab across frames, as the table's own is (`VBGUI-13`).
#[derive(Debug, Default)]
pub struct TableState {
    editor: TableEditorState,
    /// The rows `egui_deferred_table` says are selected, as it selects them, so the buttons change them.
    selected_rows: BTreeSet<usize>,
}

/// A table's commands, for the tab to handle with [`update_table`] (`VBGUI-13`).
#[derive(Debug, Clone)]
pub enum TableUiCommand {
    /// The value of the cell being edited was typed, or chosen.
    CellEdited(CellIndex, CellEdit),
    /// The edit was finished, with Enter, so it's written.
    EditSubmitted,
    /// The edit was abandoned, with Escape, leaving the value as it was.
    EditCancelled,
    /// `egui_deferred_table`'s rows selected, as the user selected them.
    RowsSelected(BTreeSet<usize>),
    /// A change to make, from a button, or an edit finished by clicking elsewhere.
    Change(TableChange),
}

/// Handles a table's command, changing the cell edited, or the rows selected, and returns the change to make, if any
/// (`VBGUI-13`).
pub fn update_table(state: &mut TableState, command: TableUiCommand) -> Option<TableChange> {
    let editor = &mut state.editor;
    match command {
        TableUiCommand::CellEdited(cell, edit) => {
            if let Some(CellEditState::Editing(editing, current, _)) = &mut editor.state
                && *editing == cell
            {
                *current = edit;
            }
            None
        }
        TableUiCommand::EditSubmitted => match editor.state.take() {
            Some(CellEditState::Editing(cell, edit, edited)) => {
                editor.state = Some(CellEditState::Pivot(cell));
                edited.change(edit)
            }
            other => {
                editor.state = other;
                None
            }
        },
        TableUiCommand::EditCancelled => {
            if let Some(CellEditState::Editing(cell, ..)) = &editor.state {
                editor.state = Some(CellEditState::Pivot(*cell));
            }
            None
        }
        TableUiCommand::RowsSelected(rows) => {
            state.selected_rows = rows;
            None
        }
        TableUiCommand::Change(change) => Some(change),
    }
}

/// The edit of the row's value in the column, as it is, if it can be edited.
fn cell_edit(row: &TableRow, column: &ColumnValue) -> Option<CellEdit> {
    let TableRow::PartMappings(item) = row;
    match column {
        ColumnValue::Eda => Some(CellEdit::Eda(item.eda_tool)),
        ColumnValue::Pattern(field_name) => Some(CellEdit::Pattern(
            item.criteria
                .iter()
                .find(|criterion| criterion.field_name == *field_name)
                .map(|criterion| criterion.pattern.clone())
                .unwrap_or_else(|| Pattern::parse("")),
        )),
        ColumnValue::Manufacturer => Some(CellEdit::Text(item.part.manufacturer.clone())),
        ColumnValue::Mpn => Some(CellEdit::Text(item.part.mpn.clone())),
        _ => None,
    }
}

/// The row with the edit of its value in the column.
fn with_edit(row: &TableRow, column: &ColumnValue, edit: CellEdit) -> TableRow {
    let mut row = row.clone();
    let TableRow::PartMappings(item) = &mut row;
    match (column, edit) {
        (ColumnValue::Eda, CellEdit::Eda(eda_tool)) => item.eda_tool = eda_tool,
        (ColumnValue::Pattern(field_name), CellEdit::Pattern(pattern)) => {
            match item
                .criteria
                .iter_mut()
                .find(|criterion| criterion.field_name == *field_name)
            {
                Some(criterion) => criterion.pattern = pattern,
                // a pattern left empty isn't one the row didn't have.
                None if pattern.text().is_empty() => {}
                None => item.criteria.push(FieldPattern {
                    field_name: field_name.clone(),
                    pattern,
                }),
            }
        }
        (ColumnValue::Manufacturer, CellEdit::Text(text)) => item.part.manufacturer = text,
        (ColumnValue::Mpn, CellEdit::Text(text)) => item.part.mpn = text,
        _ => {}
    }
    row
}

/// A new part mapping, of the EDA tool, with no patterns and no part, to be filled in (`VBGUI-13`).
pub fn new_part_mapping(eda_tool: EdaTool) -> TableRow {
    TableRow::PartMappings(PartMappingsViewItem {
        eda_tool,
        criteria: vec![],
        part: Part::new(String::new(), String::new()),
    })
}

const EDA_TOOLS: [EdaTool; 3] = [EdaTool::DipTrace, EdaTool::KiCad, EdaTool::EasyEda];

/// As `STORE-17` names it, and the data source has it.
fn eda_tool_name(eda_tool: EdaTool) -> &'static str {
    match eda_tool {
        EdaTool::DipTrace => "DipTrace",
        EdaTool::KiCad => "KiCad",
        EdaTool::EasyEda => "EasyEda",
    }
}

/// Edits a cell, sending each change of its value as a command, so the tab changes what's edited (`VBGUI-13`).
struct TableEditor<'a> {
    send: &'a dyn Fn(TableUiCommand),
}

impl EditableTableRenderer<TableDataSource> for TableEditor<'_> {
    type Value = EditedRow;
    type ItemState = CellEdit;

    /// A cell whose column the data source has, so a change can name it (`STORE-19`).
    fn build_item_state(&self, cell_index: CellIndex, source: &mut TableDataSource) -> Option<(CellEdit, EditedRow)> {
        let column = source
            .view
            .columns
            .get(cell_index.column)?;
        column.reference.as_ref()?;
        let (key, row) = source.view.row(cell_index.row)?;
        let edit = cell_edit(&row, &column.value)?;
        Some((edit, EditedRow {
            key,
            row,
            column: column.value.clone(),
        }))
    }

    /// Finished by clicking elsewhere.
    fn on_edit_complete(
        &mut self,
        _cell_index: CellIndex,
        edit: CellEdit,
        edited: EditedRow,
        _source: &mut TableDataSource,
    ) {
        if let Some(change) = edited.change(edit) {
            (self.send)(TableUiCommand::Change(change));
        }
    }

    fn render_cell_editor(
        &self,
        ui: &mut Ui,
        cell_index: &CellIndex,
        edit: &mut CellEdit,
        _edited: &EditedRow,
        _source: &mut TableDataSource,
    ) {
        let edited = |edit: CellEdit| (self.send)(TableUiCommand::CellEdited(*cell_index, edit));
        match edit {
            CellEdit::Pattern(pattern) => {
                let color = pattern_color(pattern, ui.visuals());
                if let Some(text) = edit_text(ui, pattern.text(), Some(color), self.send) {
                    edited(CellEdit::Pattern(Pattern::parse(&text)));
                }
            }
            CellEdit::Text(text) => {
                if let Some(text) = edit_text(ui, text, None, self.send) {
                    edited(CellEdit::Text(text));
                }
            }
            CellEdit::Eda(eda_tool) => {
                let mut chosen = *eda_tool;
                ComboBox::from_id_salt(ui.id().with("eda_tool"))
                    .selected_text(eda_tool_name(chosen))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for eda_tool in EDA_TOOLS {
                            ui.selectable_value(&mut chosen, eda_tool, eda_tool_name(eda_tool));
                        }
                    });
                if chosen != *eda_tool {
                    edited(CellEdit::Eda(chosen));
                }
            }
        }
    }
}

/// A text field of the value being edited, focused, the edit finished with Enter, and abandoned with Escape. Returns
/// the text, if it was changed.
fn edit_text(ui: &mut Ui, text: &str, color: Option<Color32>, send: &dyn Fn(TableUiCommand)) -> Option<String> {
    let mut text = text.to_string();
    let mut field = TextEdit::singleline(&mut text)
        .frame(egui::Frame::NONE)
        .desired_width(f32::INFINITY);
    if let Some(color) = color {
        field = field.text_color(color);
    }
    let response = ui.add(field);
    match response.lost_focus() {
        true if ui.input(|input| input.key_pressed(Key::Escape)) => send(TableUiCommand::EditCancelled),
        true if ui.input(|input| input.key_pressed(Key::Enter)) => send(TableUiCommand::EditSubmitted),
        true => {}
        false => response.request_focus(),
    }
    response.changed().then_some(text)
}

/// The buttons that change the table's rows (`VBGUI-13`): "Add row" adds `new_row`, if there's one, and the others
/// change the row selected, if there's one, and only one, disabled while `changing`, and the row's while a cell is being
/// edited.
pub fn show_row_buttons(
    ui: &mut Ui,
    view: &TableView,
    state: &TableState,
    new_row: Option<TableRow>,
    changing: bool,
    send: &dyn Fn(TableUiCommand),
) {
    let row_count = view.table.len();
    // FUTURE several rows are removed at once.
    let selected = match state.selected_rows.len() {
        1 => state
            .selected_rows
            .first()
            .and_then(|&index| view.row(index).map(|row| (index, row))),
        _ => None,
    };
    let editing = matches!(state.editor.state, Some(CellEditState::Editing(..)));
    let row_enabled = !changing && !editing;
    let movable = view.ordering != RowOrdering::Unsupported;
    let change = |change: TableChange| send(TableUiCommand::Change(change));

    ui.horizontal(|ui| {
        let add = Button::new(tr!("data-table-button-add-row"));
        if ui
            .add_enabled(!changing && new_row.is_some(), add)
            .clicked()
            && let Some(row) = new_row
        {
            // FUTURE the row added is brought into view, and selected, which needs `egui_deferred_table` to let it
            // be, `VBGUI-13`'s target.
            change(TableChange::Add {
                row,
            });
        }

        let remove = Button::new(tr!("data-table-button-remove-row"));
        if ui
            .add_enabled(row_enabled && selected.is_some(), remove)
            .clicked()
            && let Some((_index, (key, row))) = selected.clone()
        {
            change(TableChange::Remove {
                key,
                row,
            });
        }

        let up = selected
            .clone()
            .filter(|(index, _)| movable && *index > 0);
        let move_up = Button::new(tr!("data-table-button-move-up"));
        if ui
            .add_enabled(row_enabled && up.is_some(), move_up)
            .clicked()
            && let Some((index, (key, row))) = up
        {
            change(TableChange::Move {
                key,
                row,
                before: view.row(index - 1),
            });
        }

        let down = selected.filter(|(index, _)| movable && index + 1 < row_count);
        let move_down = Button::new(tr!("data-table-button-move-down"));
        if ui
            .add_enabled(row_enabled && down.is_some(), move_down)
            .clicked()
            && let Some((index, (key, row))) = down
        {
            // before the row after the next, or to the end, if the next is the last.
            change(TableChange::Move {
                key,
                row,
                before: view.row(index + 2),
            });
        }
    });
}

/// How a table whose kind can be changed is edited: its state, kept by the tab, and where its commands are sent.
pub struct TableEditing<'a> {
    pub state: &'a mut TableState,
    pub send: &'a dyn Fn(TableUiCommand),
}

impl DeferredTableDataSource for TableDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.view.table.len(),
            column_count: self.view.columns.len(),
        }
    }
}

struct TableRenderer {}

impl DeferredTableRenderer<TableDataSource> for TableRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &TableDataSource) {
        let Some(column) = source
            .view
            .columns
            .get(cell_index.column)
        else {
            return;
        };
        match source
            .view
            .table
            .cell(cell_index.row, &column.value)
        {
            Cell::Text(text) => {
                ui.label(text.as_ref());
            }
            Cell::Pattern(pattern) => show_pattern(ui, pattern),
        }
    }
}

/// A pattern, coloured by what it is, as it was parsed when it was read, not each frame. A regular expression that isn't
/// valid says why when it's hovered.
fn show_pattern(ui: &mut Ui, pattern: &Pattern) {
    let response = ui.colored_label(pattern_color(pattern, ui.visuals()), pattern.text());
    if let Pattern::Regex {
        regex: Err(error), ..
    } = pattern
    {
        response.on_hover_text(error.to_string());
    }
}

/// Green for a valid regular expression, red for one that isn't, and blue for a value matched exactly.
fn pattern_color(pattern: &Pattern, visuals: &Visuals) -> Color32 {
    match pattern {
        Pattern::Regex {
            regex: Ok(_), ..
        } => match visuals.dark_mode {
            true => Color32::LIGHT_GREEN,
            false => Color32::DARK_GREEN,
        },
        Pattern::Regex {
            regex: Err(_), ..
        } => visuals.error_fg_color,
        Pattern::Exact(_) => match visuals.dark_mode {
            true => Color32::LIGHT_BLUE,
            false => Color32::DARK_BLUE,
        },
    }
}

/// Shows the table, a row for each of the data source's records, with its columns as the headers, each resizable, taking
/// all the space left in the `Ui`, so it's resized with its container. Its cells are edited if it's `editing`, and its
/// kind can be changed (`VBGUI-13`), else it's read-only.
#[profiling::function]
pub fn show_table(ui: &mut Ui, id: egui::Id, view: Arc<TableView>, editing: Option<TableEditing>) {
    let mut column_parameters: Vec<AxisParameters> = view
        .columns
        .iter()
        .map(|column| {
            AxisParameters::default()
                .default_dimension(150.0)
                .resizable(true)
                .name(column.name.clone())
        })
        .collect();
    if let Some(last) = column_parameters.pop() {
        column_parameters.push(last.expandable(true));
    }

    super::in_space_left(ui, |ui| {
        show_table_in(ui, id, view, editing, &column_parameters);
    });
}

fn show_table_in(
    ui: &mut Ui,
    id: egui::Id,
    view: Arc<TableView>,
    editing: Option<TableEditing>,
    column_parameters: &Vec<AxisParameters>,
) {
    let mut renderer = TableRenderer {};
    let table = DeferredTable::new(id)
        .min_size((100.0, 100.0).into())
        .column_parameters(column_parameters);

    // FUTURE selecting a row selects the corresponding rows of other tables, so the actions will be handled.
    match editing.filter(|_| view.table.is_changeable()) {
        Some(editing) => {
            let mut source = TableDataSource {
                view,
            };
            let mut editor = TableEditor {
                send: editing.send,
            };
            // FUTURE a row is selected when one of its cells is clicked, which needs `egui_deferred_table` to let it
            // be, `VBGUI-13`'s target.
            let (_response, actions) =
                table.show_and_edit(ui, &mut source, &mut renderer, &mut editor, &mut editing.state.editor);
            for action in actions {
                if let Action::RowSelectionChanged {
                    selection,
                } = action
                {
                    (editing.send)(TableUiCommand::RowsSelected(selection));
                }
            }
        }
        None => {
            let mut source = TableDataSource {
                view,
            };
            let (_response, _actions) = table.show(ui, &mut source, &mut renderer);
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::Visuals;
    use egui_deferred_table::{CellEditState, CellIndex};
    use stores_app::{
        ColumnValue, EdaTool, FieldPattern, Part, PartMappingsViewItem, Pattern, RowKey, TableChange, TableRow,
    };

    use super::{CellEdit, EditedRow, TableEditorState, TableState, TableUiCommand, pattern_color, update_table};

    fn row(name: &str, mpn: &str) -> TableRow {
        TableRow::PartMappings(PartMappingsViewItem {
            eda_tool: EdaTool::DipTrace,
            criteria: vec![FieldPattern {
                field_name: "name".to_string(),
                pattern: Pattern::parse(name),
            }],
            part: Part::new("MFR1".to_string(), mpn.to_string()),
        })
    }

    const CELL: CellIndex = CellIndex {
        row: 1,
        column: 3,
    };

    /// The MPN of the row `1`, `R2`'s, being edited to `edit`.
    fn editing(edit: CellEdit) -> TableState {
        TableState {
            editor: TableEditorState {
                state: Some(CellEditState::Editing(CELL, edit, EditedRow {
                    key: RowKey("1".to_string()),
                    row: row("R2", "MPN2"),
                    column: ColumnValue::Mpn,
                })),
            },
            ..TableState::default()
        }
    }

    /// `VBGUI-13`, an edit finished with Enter makes the change of the row as it was, to the row with the value, and
    /// keeps the cell chosen, so a click on it edits it again.
    #[test]
    fn an_edit_makes_the_change_of_the_row_as_it_was() {
        let mut state = editing(CellEdit::Text("MPN2".to_string()));

        let change = update_table(
            &mut state,
            TableUiCommand::CellEdited(CELL, CellEdit::Text("MPN5".to_string())),
        );
        assert_eq!(change, None);
        let change = update_table(&mut state, TableUiCommand::EditSubmitted);

        assert_eq!(
            change,
            Some(TableChange::Update {
                key: RowKey("1".to_string()),
                old: row("R2", "MPN2"),
                new: row("R2", "MPN5"),
            })
        );
        assert!(matches!(state.editor.state, Some(CellEditState::Pivot(CELL))));
    }

    /// `VBGUI-13`, an edit leaving the value as it was, or abandoned with Escape, makes no change.
    #[test]
    fn an_edit_leaving_the_value_or_abandoned_makes_no_change() {
        let mut state = editing(CellEdit::Text("MPN2".to_string()));
        assert_eq!(update_table(&mut state, TableUiCommand::EditSubmitted), None);
        assert!(matches!(state.editor.state, Some(CellEditState::Pivot(CELL))));

        let mut state = editing(CellEdit::Text("MPN5".to_string()));
        assert_eq!(update_table(&mut state, TableUiCommand::EditCancelled), None);
        assert!(matches!(state.editor.state, Some(CellEditState::Pivot(CELL))));
        // finishing it once it's abandoned makes no change either.
        assert_eq!(update_table(&mut state, TableUiCommand::EditSubmitted), None);

        // a pattern left empty, of a field the row had none of, isn't one.
        let edited = EditedRow {
            key: RowKey("1".to_string()),
            row: row("R2", "MPN2"),
            column: ColumnValue::Pattern("value".to_string()),
        };
        assert_eq!(edited.change(CellEdit::Pattern(Pattern::parse(""))), None);
    }

    /// `VBGUI-13`, the rows `egui_deferred_table` says are selected are kept, as it says, for the buttons, and a change
    /// is made as it's asked for, leaving them, as the table does.
    #[test]
    fn the_rows_selected_are_kept_as_the_table_says() {
        let mut state = TableState::default();

        let change = update_table(&mut state, TableUiCommand::RowsSelected([0, 2].into()));
        assert_eq!(change, None);
        assert_eq!(state.selected_rows, [0, 2].into());

        let remove = TableChange::Remove {
            key: RowKey("0".to_string()),
            row: row("R1", "MPN1"),
        };
        let change = update_table(&mut state, TableUiCommand::Change(remove.clone()));
        assert_eq!(change, Some(remove));
        assert_eq!(state.selected_rows, [0, 2].into());
    }

    /// `VBGUI-11`, a pattern's colour says what it is, in either theme.
    #[test]
    fn a_patterns_colour_says_what_it_is() {
        for visuals in [Visuals::dark(), Visuals::light()] {
            let exact = pattern_color(&Pattern::parse("RES_0402"), &visuals);
            let valid = pattern_color(&Pattern::parse("/RES_.*/"), &visuals);
            let invalid = pattern_color(&Pattern::parse("/RES_[/"), &visuals);

            assert_eq!(invalid, visuals.error_fg_color);
            assert!(valid.g() > valid.r() && valid.g() > valid.b(), "green: {valid:?}");
            assert!(exact.b() > exact.r() && exact.b() > exact.g(), "blue: {exact:?}");
        }
    }
}
