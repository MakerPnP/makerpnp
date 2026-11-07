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
    Cell, ColumnValue, EdaTool, FieldPattern, Part, PartMappingsViewItem, Pattern, Revision, RowKey, RowOrdering,
    TableChange, TableRow, TableView,
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
    /// The rows selected, by their places, which `egui_deferred_table` changes as the user selects them, and the GUI
    /// changes after a row's added, moved or removed, so the buttons change them.
    selected_rows: BTreeSet<usize>,
    /// The row to select once the change the row buttons made is shown.
    after_change: Option<AfterChange>,
    /// The row to bring into view, once, as it was added or moved.
    scroll_to_row: Option<usize>,
}

/// The row to select once a change is shown, and the revision of the view the change was made to, so a change that
/// wasn't made, as the view is the same, changes nothing (`VBGUI-13`).
#[derive(Debug, Clone, PartialEq)]
pub struct AfterChange {
    revision: Revision,
    select: SelectAfterChange,
}

#[derive(Debug, Clone, PartialEq)]
enum SelectAfterChange {
    /// The row, as it's to be shown, a row added, or moved, nearest the place it's to be in, as other rows may be the
    /// same as it.
    Row { row: TableRow, place: usize },
    /// No row, as the row selected was removed.
    NoRow,
}

impl AfterChange {
    /// The rows to select, now the change is shown in the view, if they change.
    fn shown(&self, view: &TableView) -> ShownAfterChange {
        if view.revision == self.revision {
            return ShownAfterChange::Unchanged;
        }
        match &self.select {
            SelectAfterChange::Row {
                row,
                place,
            } => (0..view.table.len())
                .filter(|&index| {
                    view.table
                        .row(index)
                        .is_some_and(|shown| same_row(&shown, row))
                })
                .min_by_key(|index| index.abs_diff(*place))
                .map_or(ShownAfterChange::Unchanged, ShownAfterChange::Row),
            SelectAfterChange::NoRow => ShownAfterChange::NoRow,
        }
    }
}

/// Whether the rows are the same, as they're read back: a row is read with a pattern of each of the data source's
/// fields, an empty one for a field it had none of, which isn't one, as an edit leaving it empty isn't.
fn same_row(a: &TableRow, b: &TableRow) -> bool {
    let (TableRow::PartMappings(a), TableRow::PartMappings(b)) = (a, b);
    fn patterns(item: &PartMappingsViewItem) -> Vec<(&str, &str)> {
        let mut patterns = item
            .criteria
            .iter()
            .filter(|criterion| !criterion.pattern.text().is_empty())
            .map(|criterion| (criterion.field_name.as_str(), criterion.pattern.text()))
            .collect::<Vec<_>>();
        patterns.sort();
        patterns
    }
    a.eda_tool == b.eda_tool && a.part == b.part && patterns(a) == patterns(b)
}

/// The rows to select once a change is shown (`VBGUI-13`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShownAfterChange {
    /// The rows selected are kept, as the change wasn't made, or its row isn't shown.
    Unchanged,
    /// Only the row at the place, which is brought into view.
    Row(usize),
    /// No row.
    NoRow,
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
    /// A change to make, an edit finished by clicking elsewhere.
    Change(TableChange),
    /// A change to make, from a row button, and the row to select once it's shown.
    ChangeRows(TableChange, AfterChange),
    /// The change the row buttons made is shown, or wasn't made, so the rows are selected as it says.
    ChangeShown(ShownAfterChange),
    /// The table was asked to bring the row into view.
    ScrolledToRow,
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
        TableUiCommand::ChangeRows(change, after_change) => {
            state.after_change = Some(after_change);
            Some(change)
        }
        TableUiCommand::ChangeShown(shown) => {
            state.after_change = None;
            match shown {
                ShownAfterChange::Unchanged => return None,
                ShownAfterChange::Row(index) => {
                    state.selected_rows = BTreeSet::from([index]);
                    state.scroll_to_row = Some(index);
                }
                ShownAfterChange::NoRow => state.selected_rows.clear(),
            }
            // the cell chosen is of the row that was in its place, so a click on it chooses it, rather than editing a
            // row that may not be the one chosen.
            if matches!(state.editor.state, Some(CellEditState::Pivot(_))) {
                state.editor.state = None;
            }
            None
        }
        TableUiCommand::ScrolledToRow => {
            state.scroll_to_row = None;
            None
        }
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
    // each change selects the row it's of once it's shown, the row added, or moved, where it's to be, and no row once
    // the row selected is removed.
    let change = |change: TableChange, select: SelectAfterChange| {
        send(TableUiCommand::ChangeRows(change, AfterChange {
            revision: view.revision.clone(),
            select,
        }))
    };
    let select_row = |row: &TableRow, place: usize| SelectAfterChange::Row {
        row: row.clone(),
        place,
    };

    ui.horizontal(|ui| {
        let add = Button::new(tr!("data-table-button-add-row"));
        if ui
            .add_enabled(!changing && new_row.is_some(), add)
            .clicked()
            && let Some(row) = new_row
        {
            // after the others.
            let select = select_row(&row, row_count);
            change(
                TableChange::Add {
                    row,
                },
                select,
            );
        }

        let remove = Button::new(tr!("data-table-button-remove-row"));
        if ui
            .add_enabled(row_enabled && selected.is_some(), remove)
            .clicked()
            && let Some((_index, (key, row))) = selected.clone()
        {
            change(
                TableChange::Remove {
                    key,
                    row,
                },
                SelectAfterChange::NoRow,
            );
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
            let select = select_row(&row, index - 1);
            change(
                TableChange::Move {
                    key,
                    row,
                    before: view.row(index - 1),
                },
                select,
            );
        }

        let down = selected.filter(|(index, _)| movable && index + 1 < row_count);
        let move_down = Button::new(tr!("data-table-button-move-down"));
        if ui
            .add_enabled(row_enabled && down.is_some(), move_down)
            .clicked()
            && let Some((index, (key, row))) = down
        {
            // before the row after the next, or to the end, if the next is the last.
            let select = select_row(&row, index + 1);
            change(
                TableChange::Move {
                    key,
                    row,
                    before: view.row(index + 2),
                },
                select,
            );
        }
    });
}

/// How a table whose kind can be changed is edited: its state, kept by the tab, and where its commands are sent.
pub struct TableEditing<'a> {
    pub state: &'a mut TableState,
    pub send: &'a dyn Fn(TableUiCommand),
    /// Whether a change is being written, or the table read again after one, so the view isn't yet as the change left
    /// it.
    pub busy: bool,
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

    let mut renderer = TableRenderer {};
    // `egui_deferred_table` takes the space left below what's above it (`ISSUE-25`).
    let table = DeferredTable::new(id)
        .min_size((100.0, 100.0).into())
        .column_parameters(&column_parameters)
        .select_rows_on_cell_click();

    // FUTURE selecting a row selects the corresponding rows of other tables, so the actions will be handled.
    match editing.filter(|_| view.table.is_changeable()) {
        Some(editing) => {
            let send = editing.send;
            let state = editing.state;
            // once the change the row buttons made is shown, the rows are selected as it says, by `update_table`.
            if let Some(after_change) = &state.after_change
                && !editing.busy
            {
                send(TableUiCommand::ChangeShown(after_change.shown(&view)));
            }
            let mut table = table.row_selection(&mut state.selected_rows);
            if let Some(row) = state.scroll_to_row {
                table = table.scroll_to_row(row, None);
                send(TableUiCommand::ScrolledToRow);
            }

            let mut source = TableDataSource {
                view,
            };
            let mut editor = TableEditor {
                send,
            };
            let (_response, actions) =
                table.show_and_edit(ui, &mut source, &mut renderer, &mut editor, &mut state.editor);
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
        ColumnValue, EdaTool, FieldPattern, Part, PartMappingsViewItem, Pattern, Revision, RowKey, RowOrdering, Source,
        Table, TableChange, TableKind, TableRow, TableView,
    };

    use super::{
        AfterChange, CellEdit, EditedRow, SelectAfterChange, ShownAfterChange, TableEditorState, TableState,
        TableUiCommand, pattern_color, update_table,
    };

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

    /// `VBGUI-13`, the rows `egui_deferred_table` says are selected are kept, as it says, for the buttons, and an edit's
    /// change is made as it's asked for, leaving them.
    #[test]
    fn the_rows_selected_are_kept_as_the_table_says() {
        let mut state = TableState::default();

        let change = update_table(&mut state, TableUiCommand::RowsSelected([0, 2].into()));
        assert_eq!(change, None);
        assert_eq!(state.selected_rows, [0, 2].into());

        let update = TableChange::Update {
            key: RowKey("0".to_string()),
            old: row("R1", "MPN1"),
            new: row("R1", "MPN5"),
        };
        let change = update_table(&mut state, TableUiCommand::Change(update.clone()));
        assert_eq!(change, Some(update));
        assert_eq!(state.selected_rows, [0, 2].into());
        assert_eq!(state.after_change, None);
    }

    fn view(revision: &str, rows: &[TableRow]) -> TableView {
        TableView {
            kind: TableKind::PartMappings,
            source: Source::File("mappings.csv".into()),
            revision: Revision(revision.to_string()),
            columns: vec![],
            table: Table::PartMappings(
                rows.iter()
                    .map(|TableRow::PartMappings(item)| item.clone())
                    .collect(),
            ),
            keys: (0..rows.len())
                .map(|index| RowKey(index.to_string()))
                .collect(),
            ordering: RowOrdering::Position,
        }
    }

    fn after_change(revision: &str, select: SelectAfterChange) -> AfterChange {
        AfterChange {
            revision: Revision(revision.to_string()),
            select,
        }
    }

    fn select_row(row: TableRow, place: usize) -> SelectAfterChange {
        SelectAfterChange::Row {
            row,
            place,
        }
    }

    /// `VBGUI-13`, once a change is shown, the row it's of is selected, the one nearest the place it's to be, as rows
    /// can be the same, no row once the row selected was removed, and the rows selected are kept if the change wasn't
    /// made, as the view's revision is the one it was made to, or its row isn't shown.
    #[test]
    fn the_row_a_change_is_of_is_selected_once_its_shown() {
        let empty = row("", "");
        let changed = view("2", &[
            row("R1", "MPN1"),
            empty.clone(),
            row("R2", "MPN2"),
            empty.clone(),
        ]);

        // the row added, after the others, is the last of the rows that are the same.
        assert_eq!(
            after_change("1", select_row(empty.clone(), 3)).shown(&changed),
            ShownAfterChange::Row(3)
        );
        // the row moved up, to the second place.
        assert_eq!(
            after_change("1", select_row(empty.clone(), 1)).shown(&changed),
            ShownAfterChange::Row(1)
        );
        assert_eq!(
            after_change("1", SelectAfterChange::NoRow).shown(&changed),
            ShownAfterChange::NoRow
        );

        // not made.
        assert_eq!(
            after_change("2", select_row(empty, 3)).shown(&changed),
            ShownAfterChange::Unchanged
        );
        assert_eq!(
            after_change("2", SelectAfterChange::NoRow).shown(&changed),
            ShownAfterChange::Unchanged
        );
        // made since by someone else, without the row.
        assert_eq!(
            after_change("1", select_row(row("R3", "MPN3"), 3)).shown(&changed),
            ShownAfterChange::Unchanged
        );

        // a row added with no patterns is read back with an empty pattern of each field, which isn't one.
        let added = TableRow::PartMappings(PartMappingsViewItem {
            eda_tool: EdaTool::DipTrace,
            criteria: vec![],
            part: Part::new("MFR1".to_string(), String::new()),
        });
        assert_eq!(
            after_change("1", select_row(added, 3)).shown(&changed),
            ShownAfterChange::Row(3)
        );
    }

    /// `VBGUI-13`, a row button's change is made as it's asked for, and once it's shown, the row it's of is selected,
    /// alone, and brought into view, once, or no row is.
    #[test]
    fn a_row_buttons_change_selects_its_row_once_its_shown() {
        let mut state = TableState::default();
        update_table(&mut state, TableUiCommand::RowsSelected([0, 2].into()));
        state.editor.state = Some(CellEditState::Pivot(CELL));

        let add = TableChange::Add {
            row: row("", ""),
        };
        let after = after_change("1", select_row(row("", ""), 2));
        let change = update_table(&mut state, TableUiCommand::ChangeRows(add.clone(), after.clone()));
        assert_eq!(change, Some(add));
        assert_eq!(state.after_change, Some(after));
        // until it's shown, the rows selected, and the cell chosen, are kept.
        assert_eq!(state.selected_rows, [0, 2].into());
        assert!(matches!(state.editor.state, Some(CellEditState::Pivot(CELL))));

        assert_eq!(
            update_table(&mut state, TableUiCommand::ChangeShown(ShownAfterChange::Row(2))),
            None
        );
        assert_eq!(state.after_change, None);
        assert_eq!(state.selected_rows, [2].into());
        assert_eq!(state.scroll_to_row, Some(2));
        // the cell chosen was of the row in its place before the change, so none is.
        assert!(state.editor.state.is_none());
        update_table(&mut state, TableUiCommand::ScrolledToRow);
        assert_eq!(state.scroll_to_row, None);

        // kept, if it wasn't made.
        state.editor.state = Some(CellEditState::Pivot(CELL));
        update_table(&mut state, TableUiCommand::ChangeShown(ShownAfterChange::Unchanged));
        assert_eq!(state.selected_rows, [2].into());
        assert_eq!(state.scroll_to_row, None);
        assert!(matches!(state.editor.state, Some(CellEditState::Pivot(CELL))));

        update_table(&mut state, TableUiCommand::ChangeShown(ShownAfterChange::NoRow));
        assert_eq!(state.selected_rows, [].into());
        assert_eq!(state.scroll_to_row, None);
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
