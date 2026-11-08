//! A table of a data source's records, as the stores core shows any data source other than parts (`STORE-17`), each
//! value shown as what it is, a pattern by its kind (`VBGUI-11`), and, if its kind can be changed, edited in its cells,
//! and its rows added, removed and moved: part mappings (`VBGUI-13`), substitutions (`VBGUI-14`) and assembly rules
//! (`VBGUI-15`).

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Button, Color32, ComboBox, Key, Label, RichText, Sense, TextEdit, Ui, Visuals};
use egui_deferred_table::{
    Action, AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer,
    EditableTableRenderer, EditorState, TableDimensions,
};
use egui_i18n::tr;
use stores_app::{
    AssemblyRule, AssemblyRulesViewItem, Cell, ColumnValue, EdaSubstitutionRuleTransformItem, EdaTool, FieldPattern,
    Part, PartMappingsViewItem, Pattern, Revision, RowKey, RowOrdering, SubstitutionsViewItem, Table, TableChange,
    TableColumn, TableKind, TableRow, TableView,
};

/// The view the stores core sent, shared with every other table showing the same data source, not a copy of it.
struct TableDataSource {
    view: Arc<TableView>,
}

impl TableDataSource {
    /// The view's column the table's column shows, none if it's the status column.
    fn column(&self, index: usize) -> Option<&TableColumn> {
        view_column(&self.view, index)
    }
}

/// Whether the table has a status column, first, shown only by the GUI, as its rows' kind can be changed, so each can
/// have problems (`VBGUI-13`).
fn has_status(view: &TableView) -> bool {
    is_changed_here(view)
}

/// Whether the GUI changes the table: part mappings (`VBGUI-13`), substitutions (`VBGUI-14`) and assembly rules
/// (`VBGUI-15`).
pub fn is_changed_here(view: &TableView) -> bool {
    matches!(
        view.table,
        Table::PartMappings(_) | Table::Substitutions(_) | Table::AssemblyRules(_)
    )
}

/// The view's column the table's column shows, after the status column if there's one.
fn view_column(view: &TableView, index: usize) -> Option<&TableColumn> {
    let index = match has_status(view) {
        true => index.checked_sub(1)?,
        false => index,
    };
    view.columns.get(index)
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
/// as the user saw it (`STORE-19`), and the EDA tools whose columns the table didn't have then (`STORE-21`).
#[derive(Debug, Clone)]
pub struct EditedRow {
    key: RowKey,
    row: TableRow,
    column: ColumnValue,
    tools_without_columns: Vec<EdaTool>,
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

    /// The command that makes the edit's change: the row's change, or, if it's given an EDA tool whose columns the
    /// table doesn't have, their adding, the change of its tool pending until they're shown (`VBGUI-13`).
    fn command(self, edit: CellEdit) -> Option<TableUiCommand> {
        match edit {
            CellEdit::Eda(eda_tool)
                if self
                    .tools_without_columns
                    .contains(&eda_tool) =>
            {
                (self.row.eda_tool() != Some(eda_tool)).then(|| {
                    TableUiCommand::ChangeAfterColumns(PendingChange {
                        eda_tool,
                        change: PendingRowChange::Tool {
                            key: self.key,
                            row: self.row,
                        },
                    })
                })
            }
            edit => self
                .change(edit)
                .map(TableUiCommand::Change),
        }
    }
}

/// Whether the table has every one of the EDA tool's fields' columns, so a row of it can be given each of its patterns,
/// and, a substitution, each of its values (`STORE-21`).
fn has_columns(view: &TableView, eda_tool: EdaTool) -> bool {
    let has_column = |value: ColumnValue| {
        view.columns
            .iter()
            .any(|column| column.reference.is_some() && column.value == value)
    };
    let substitutions = matches!(view.table, Table::Substitutions(_));
    eda_tool
        .field_names()
        .iter()
        .all(|field_name| {
            has_column(ColumnValue::Pattern(field_name.to_string()))
                && (!substitutions || has_column(ColumnValue::Substitute(field_name.to_string())))
        })
}

/// A row's change pending until the EDA tool's columns, which it needs, are added, and the table's read again
/// (`VBGUI-13`, `STORE-21`).
#[derive(Debug, Clone, PartialEq)]
pub struct PendingChange {
    eda_tool: EdaTool,
    change: PendingRowChange,
}

#[derive(Debug, Clone, PartialEq)]
enum PendingRowChange {
    /// The row given the EDA tool, the row as it was shown when the change was made.
    Tool { key: RowKey, row: TableRow },
    /// The row added, after the others, which is selected once it's shown.
    Add { row: TableRow },
}

impl PendingChange {
    /// The change, now the columns are shown, of the row as it's shown now, with the row to select once it's shown, or
    /// none, if the row isn't shown as it was, as it was changed since.
    fn made(self, view: &TableView) -> Option<(TableChange, Option<AfterChange>)> {
        match self.change {
            PendingRowChange::Tool {
                key,
                row,
            } => {
                let index = view
                    .keys
                    .iter()
                    .position(|shown| *shown == key)?;
                let (key, old) = view
                    .row(index)
                    .filter(|(_key, shown)| same_row(shown, &row))?;
                let mut new = old.clone();
                match &mut new {
                    TableRow::PartMappings(item) => item.eda_tool = self.eda_tool,
                    TableRow::Substitutions(item) => item.eda_tool = self.eda_tool,
                    // an assembly rule, or a part, has no EDA tool, so has no columns to add for one.
                    TableRow::AssemblyRules(_) | TableRow::Parts(_) => return None,
                }
                Some((
                    TableChange::Update {
                        key,
                        old,
                        new,
                    },
                    None,
                ))
            }
            PendingRowChange::Add {
                row,
            } => {
                let after = AfterChange {
                    revision: view.revision.clone(),
                    select: SelectAfterChange::Row {
                        row: row.clone(),
                        place: view.table.len(),
                    },
                };
                Some((
                    TableChange::Add {
                        row,
                    },
                    Some(after),
                ))
            }
        }
    }
}

/// What a table asks the tab to have the stores core do (`VBGUI-13`).
#[derive(Debug, Clone, PartialEq)]
pub enum TableRequest {
    /// Change its rows (`STORE-19`).
    Change(TableChange),
    /// Add the EDA tool's columns the table doesn't have (`STORE-21`).
    AddColumns(EdaTool),
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
    /// The row's change to make once the columns it needs are added, and shown.
    pending: Option<PendingChange>,
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
/// fields, an empty one for a field it had none of, which isn't one, as an edit leaving it empty isn't, and likewise a
/// substitution's values and comment.
fn same_row(a: &TableRow, b: &TableRow) -> bool {
    fn patterns(criteria: &[FieldPattern]) -> Vec<(&str, &str)> {
        let mut patterns = criteria
            .iter()
            .filter(|criterion| !criterion.pattern.text().is_empty())
            .map(|criterion| (criterion.field_name.as_str(), criterion.pattern.text()))
            .collect::<Vec<_>>();
        patterns.sort();
        patterns
    }
    fn values(transforms: &[EdaSubstitutionRuleTransformItem]) -> Vec<(&str, &str)> {
        let mut values = transforms
            .iter()
            .filter(|transform| !transform.field_value.is_empty())
            .map(|transform| (transform.field_name.as_str(), transform.field_value.as_str()))
            .collect::<Vec<_>>();
        values.sort();
        values
    }
    match (a, b) {
        (TableRow::PartMappings(a), TableRow::PartMappings(b)) => {
            a.eda_tool == b.eda_tool && a.part == b.part && patterns(&a.criteria) == patterns(&b.criteria)
        }
        (TableRow::Substitutions(a), TableRow::Substitutions(b)) => {
            a.eda_tool == b.eda_tool
                && patterns(&a.criteria) == patterns(&b.criteria)
                && values(&a.transforms) == values(&b.transforms)
                && a.comment.as_deref().unwrap_or_default() == b.comment.as_deref().unwrap_or_default()
        }
        (TableRow::AssemblyRules(a), TableRow::AssemblyRules(b)) => a == b,
        _ => false,
    }
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
    /// A row's change that needs an EDA tool's columns the table doesn't have, so they're added first.
    ChangeAfterColumns(PendingChange),
    /// The columns a pending change needed are shown, or weren't added, so the change, if there's still one, of the row
    /// as it's shown now, is made, with the row to select once it's shown.
    ColumnsShown(Option<(TableChange, Option<AfterChange>)>),
}

/// Handles a table's command, changing the cell edited, or the rows selected, and returns what to have the stores core
/// do, if anything (`VBGUI-13`).
pub fn update_table(state: &mut TableState, command: TableUiCommand) -> Option<TableRequest> {
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
                edited
                    .command(edit)
                    .and_then(|command| update_table(state, command))
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
        TableUiCommand::Change(change) => Some(TableRequest::Change(change)),
        TableUiCommand::ChangeRows(change, after_change) => {
            state.after_change = Some(after_change);
            Some(TableRequest::Change(change))
        }
        TableUiCommand::ChangeAfterColumns(pending) => {
            let eda_tool = pending.eda_tool;
            state.pending = Some(pending);
            Some(TableRequest::AddColumns(eda_tool))
        }
        TableUiCommand::ColumnsShown(made) => {
            state.pending = None;
            let (change, after_change) = made?;
            state.after_change = after_change;
            Some(TableRequest::Change(change))
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
    let pattern = |criteria: &[FieldPattern], field_name: &str| {
        CellEdit::Pattern(
            criteria
                .iter()
                .find(|criterion| criterion.field_name == *field_name)
                .map(|criterion| criterion.pattern.clone())
                .unwrap_or_else(|| Pattern::parse("")),
        )
    };
    match (row, column) {
        (TableRow::PartMappings(item), ColumnValue::Eda) => Some(CellEdit::Eda(item.eda_tool)),
        (TableRow::PartMappings(item), ColumnValue::Pattern(field_name)) => Some(pattern(&item.criteria, field_name)),
        (TableRow::PartMappings(item), ColumnValue::Manufacturer) => {
            Some(CellEdit::Text(item.part.manufacturer.clone()))
        }
        (TableRow::PartMappings(item), ColumnValue::Mpn) => Some(CellEdit::Text(item.part.mpn.clone())),
        // `VBGUI-14`, a substitution's values and comment are edited as text.
        (TableRow::Substitutions(item), ColumnValue::Eda) => Some(CellEdit::Eda(item.eda_tool)),
        (TableRow::Substitutions(item), ColumnValue::Pattern(field_name)) => Some(pattern(&item.criteria, field_name)),
        (TableRow::Substitutions(item), ColumnValue::Substitute(field_name)) => Some(CellEdit::Text(
            item.transforms
                .iter()
                .find(|transform| transform.field_name == *field_name)
                .map(|transform| transform.field_value.clone())
                .unwrap_or_default(),
        )),
        (TableRow::Substitutions(item), ColumnValue::Comment) => {
            Some(CellEdit::Text(item.comment.clone().unwrap_or_default()))
        }
        // `VBGUI-15`, an assembly rule's values are edited as text.
        (TableRow::AssemblyRules(item), ColumnValue::RefDes) => Some(CellEdit::Text(item.rule.ref_des.clone())),
        (TableRow::AssemblyRules(item), ColumnValue::Manufacturer) => {
            Some(CellEdit::Text(item.rule.manufacturer.clone()))
        }
        (TableRow::AssemblyRules(item), ColumnValue::Mpn) => Some(CellEdit::Text(item.rule.mpn.clone())),
        _ => None,
    }
}

/// The row with the edit of its value in the column.
fn with_edit(row: &TableRow, column: &ColumnValue, edit: CellEdit) -> TableRow {
    let mut row = row.clone();
    match (&mut row, column, edit) {
        (TableRow::PartMappings(item), ColumnValue::Eda, CellEdit::Eda(eda_tool)) => item.eda_tool = eda_tool,
        (TableRow::PartMappings(item), ColumnValue::Pattern(field_name), CellEdit::Pattern(pattern)) => {
            set_pattern(&mut item.criteria, field_name, pattern)
        }
        (TableRow::PartMappings(item), ColumnValue::Manufacturer, CellEdit::Text(text)) => {
            item.part.manufacturer = text
        }
        (TableRow::PartMappings(item), ColumnValue::Mpn, CellEdit::Text(text)) => item.part.mpn = text,
        (TableRow::Substitutions(item), ColumnValue::Eda, CellEdit::Eda(eda_tool)) => item.eda_tool = eda_tool,
        (TableRow::Substitutions(item), ColumnValue::Pattern(field_name), CellEdit::Pattern(pattern)) => {
            set_pattern(&mut item.criteria, field_name, pattern)
        }
        (TableRow::Substitutions(item), ColumnValue::Substitute(field_name), CellEdit::Text(text)) => {
            match item
                .transforms
                .iter_mut()
                .find(|transform| transform.field_name == *field_name)
            {
                Some(transform) => transform.field_value = text,
                // a value left empty isn't one the row didn't have.
                None if text.is_empty() => {}
                None => item
                    .transforms
                    .push(EdaSubstitutionRuleTransformItem {
                        field_name: field_name.clone(),
                        field_value: text,
                    }),
            }
        }
        (TableRow::Substitutions(item), ColumnValue::Comment, CellEdit::Text(text)) => {
            // a comment left empty isn't one the row didn't have.
            if item.comment.is_some() || !text.is_empty() {
                item.comment = Some(text);
            }
        }
        (TableRow::AssemblyRules(item), ColumnValue::RefDes, CellEdit::Text(text)) => item.rule.ref_des = text,
        (TableRow::AssemblyRules(item), ColumnValue::Manufacturer, CellEdit::Text(text)) => {
            item.rule.manufacturer = text
        }
        (TableRow::AssemblyRules(item), ColumnValue::Mpn, CellEdit::Text(text)) => item.rule.mpn = text,
        _ => {}
    }
    row
}

/// Sets the field's pattern, adding it if the row has no pattern of it.
fn set_pattern(criteria: &mut Vec<FieldPattern>, field_name: &str, pattern: Pattern) {
    match criteria
        .iter_mut()
        .find(|criterion| criterion.field_name == *field_name)
    {
        Some(criterion) => criterion.pattern = pattern,
        // a pattern left empty isn't one the row didn't have.
        None if pattern.text().is_empty() => {}
        None => criteria.push(FieldPattern {
            field_name: field_name.to_string(),
            pattern,
        }),
    }
}

/// A new row of the table, to be filled in: a part mapping of the EDA tool with no patterns and no part (`VBGUI-13`),
/// a substitution of the EDA tool with no patterns, values or comment (`VBGUI-14`), or an assembly rule with no
/// designator, manufacturer or MPN, whether or not there's an EDA tool, as a rule has none (`VBGUI-15`); none if the
/// table's rows aren't added here, or need an EDA tool and there's none.
pub fn new_row(kind: TableKind, eda_tool: Option<EdaTool>) -> Option<TableRow> {
    match kind {
        TableKind::PartMappings => Some(TableRow::PartMappings(PartMappingsViewItem {
            eda_tool: eda_tool?,
            criteria: vec![],
            part: Part::new(String::new(), String::new()),
        })),
        TableKind::Substitutions => Some(TableRow::Substitutions(SubstitutionsViewItem {
            eda_tool: eda_tool?,
            criteria: vec![],
            transforms: vec![],
            comment: None,
        })),
        TableKind::AssemblyRules => Some(TableRow::AssemblyRules(AssemblyRulesViewItem {
            rule: AssemblyRule {
                ref_des: String::new(),
                manufacturer: String::new(),
                mpn: String::new(),
            },
        })),
        _ => None,
    }
}

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
        let column = source.column(cell_index.column)?;
        column.reference.as_ref()?;
        let (key, row) = source.view.row(cell_index.row)?;
        let edit = cell_edit(&row, &column.value)?;
        let tools_without_columns = EdaTool::ALL
            .into_iter()
            .filter(|&eda_tool| !has_columns(&source.view, eda_tool))
            .collect();
        Some((edit, EditedRow {
            key,
            row,
            column: column.value.clone(),
            tools_without_columns,
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
        if let Some(command) = edited.command(edit) {
            (self.send)(command);
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
                        for eda_tool in EdaTool::ALL {
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

/// The buttons that change the table's rows (`VBGUI-13`): "Add row" adds `new_row`, if there's one, adding its EDA
/// tool's columns first, if the table doesn't have them, and the others change the row selected, if there's one, and
/// only one, disabled while `changing`, or a change waits for its columns, and the row's while a cell is being edited.
pub fn show_row_buttons(
    ui: &mut Ui,
    view: &TableView,
    state: &TableState,
    new_row: Option<TableRow>,
    changing: bool,
    send: &dyn Fn(TableUiCommand),
) {
    let changing = changing || state.pending.is_some();
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
            match row
                .eda_tool()
                .filter(|&eda_tool| !has_columns(view, eda_tool))
            {
                None => {
                    // after the others.
                    let select = select_row(&row, row_count);
                    change(
                        TableChange::Add {
                            row,
                        },
                        select,
                    );
                }
                Some(eda_tool) => send(TableUiCommand::ChangeAfterColumns(PendingChange {
                    eda_tool,
                    change: PendingRowChange::Add {
                        row,
                    },
                })),
            }
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
    /// Whether the last change was refused, such as adding columns a pending change needed (`STORE-21`).
    pub refused: bool,
}

impl DeferredTableDataSource for TableDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.view.table.len(),
            column_count: self.view.columns.len() + usize::from(has_status(&self.view)),
        }
    }
}

struct TableRenderer {}

impl DeferredTableRenderer<TableDataSource> for TableRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &TableDataSource) {
        let row = source.view.table.row(cell_index.row);
        let Some(column) = source.column(cell_index.column) else {
            if let Some(row) = &row {
                show_status(ui, &Problem::of(&source.view, row));
            }
            return;
        };
        // `VBGUI-13`, a pattern's cell of a row whose kind can be changed says how the row's EDA tool uses it, even with
        // no pattern, and its manufacturer's and MPN's say whether they're empty; `VBGUI-14`, a substitution's value's
        // says whether the row's tool substitutes its field.
        let (background, text) = match (&column.value, &row) {
            (ColumnValue::Pattern(field_name), Some(TableRow::PartMappings(item))) => {
                let matched = PatternMatch::of(item.eda_tool, &item.criteria, field_name);
                (Some(matched.background()), Some(matched.text()))
            }
            (ColumnValue::Pattern(field_name), Some(TableRow::Substitutions(item))) => {
                let matched = PatternMatch::of(item.eda_tool, &item.criteria, field_name);
                (Some(matched.background()), Some(matched.text()))
            }
            (ColumnValue::Substitute(field_name), Some(TableRow::Substitutions(item))) => {
                let substituted = Substitution::of(item, field_name);
                (Some(substituted.background()), Some(substituted.text()))
            }
            (ColumnValue::Manufacturer, Some(TableRow::PartMappings(item))) => {
                (Some(Background::of_needed_value(&item.part.manufacturer)), None)
            }
            (ColumnValue::Mpn, Some(TableRow::PartMappings(item))) => {
                (Some(Background::of_needed_value(&item.part.mpn)), None)
            }
            // `VBGUI-15`, each of an assembly rule's values says whether it's empty, as every rule needs all three.
            (value, Some(TableRow::AssemblyRules(item))) => (Background::of_assembly_rule(item, value), None),
            _ => (None, None),
        };
        if let Some(background) = background {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, background.color(ui.visuals()));
        }
        let mut hover: Vec<String> = text.into_iter().collect();
        match source
            .view
            .table
            .cell(cell_index.row, &column.value)
        {
            // `VBGUI-13`, an EDA tool's text can't be selected, as it's chosen, not typed.
            Cell::Text(text) if column.value == ColumnValue::Eda => {
                ui.add(Label::new(text.as_ref()).selectable(false));
            }
            Cell::Text(text) => {
                ui.label(text.as_ref());
            }
            Cell::Pattern(pattern) => {
                show_pattern(ui, pattern);
                if let Pattern::Regex {
                    regex: Err(error), ..
                } = pattern
                {
                    hover.push(error.to_string());
                }
            }
        }
        // the whole cell, as an empty pattern's is the cell itself, and senses only hovering, so it's still clicked.
        if !hover.is_empty() {
            ui.interact(ui.max_rect(), ui.id().with("hover"), Sense::hover())
                .on_hover_text(hover.join("\n"));
        }
    }
}

/// How a row's EDA tool uses a field's pattern, the first of these that's so (`VBGUI-13`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum PatternMatch {
    /// The tool doesn't match the field, so its pattern is ignored.
    NotMatched(EdaTool),
    /// The tool matches the field, and its pattern is a regular expression that isn't valid.
    Invalid(EdaTool),
    /// The tool matches the field, and none of its fields has a pattern in the row, empty or with no column, so the row
    /// has nothing to match with.
    NoPatterns(EdaTool),
    /// The tool matches the field.
    Matched(EdaTool),
}

impl PatternMatch {
    /// Of the field's pattern of a row of the EDA tool with the patterns, a part mapping's or a substitution's.
    fn of(eda_tool: EdaTool, criteria: &[FieldPattern], field_name: &str) -> Self {
        let pattern = |field_name: &str| {
            criteria
                .iter()
                .find(|criterion| criterion.field_name == field_name)
                .map(|criterion| &criterion.pattern)
        };
        let fields = eda_tool.field_names();
        if !fields.contains(&field_name) {
            return PatternMatch::NotMatched(eda_tool);
        }
        if let Some(Pattern::Regex {
            regex: Err(_), ..
        }) = pattern(field_name)
        {
            return PatternMatch::Invalid(eda_tool);
        }
        let none = fields
            .iter()
            .all(|&field_name| pattern(field_name).is_none_or(|pattern| pattern.text().is_empty()));
        match none {
            true => PatternMatch::NoPatterns(eda_tool),
            false => PatternMatch::Matched(eda_tool),
        }
    }

    fn background(self) -> Background {
        match self {
            PatternMatch::NotMatched(_) => Background::Blue,
            PatternMatch::Invalid(_) => Background::Red,
            PatternMatch::NoPatterns(_) => Background::Orange,
            PatternMatch::Matched(_) => Background::Green,
        }
    }

    /// Said when it's hovered, so the colour isn't the only sign; an invalid expression's cell says why, too.
    fn text(self) -> String {
        match self {
            PatternMatch::NotMatched(eda_tool) => {
                tr!("data-table-pattern-not-matched", { tool: eda_tool_name(eda_tool) })
            }
            PatternMatch::NoPatterns(eda_tool) => {
                tr!("data-table-pattern-no-patterns", { tool: eda_tool_name(eda_tool) })
            }
            PatternMatch::Invalid(eda_tool) | PatternMatch::Matched(eda_tool) => {
                tr!("data-table-pattern-matched", { tool: eda_tool_name(eda_tool) })
            }
        }
    }
}

/// Whether a substitution's EDA tool substitutes a field's value, the first of these that's so, an empty value being
/// one, as a field can be substituted with nothing, unless every one of the tool's values is empty (`VBGUI-14`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Substitution {
    /// The tool doesn't substitute the field, so its value is ignored.
    NotSubstituted(EdaTool),
    /// The tool substitutes the field, and none of its fields has a value, empty or with no column.
    NoValues(EdaTool),
    /// The tool substitutes the field.
    Substituted(EdaTool),
}

impl Substitution {
    fn of(item: &SubstitutionsViewItem, field_name: &str) -> Self {
        let eda_tool = item.eda_tool;
        match eda_tool
            .field_names()
            .contains(&field_name)
        {
            false => Substitution::NotSubstituted(eda_tool),
            true if has_no_values(item) => Substitution::NoValues(eda_tool),
            true => Substitution::Substituted(eda_tool),
        }
    }

    fn background(self) -> Background {
        match self {
            Substitution::NotSubstituted(_) => Background::Blue,
            Substitution::NoValues(_) => Background::Red,
            Substitution::Substituted(_) => Background::Green,
        }
    }

    /// Said when it's hovered, so the colour isn't the only sign.
    fn text(self) -> String {
        match self {
            Substitution::NotSubstituted(eda_tool) => {
                tr!("data-table-value-not-substituted", { tool: eda_tool_name(eda_tool) })
            }
            Substitution::NoValues(eda_tool) => {
                tr!("data-table-value-no-values", { tool: eda_tool_name(eda_tool) })
            }
            Substitution::Substituted(eda_tool) => {
                tr!("data-table-value-substituted", { tool: eda_tool_name(eda_tool) })
            }
        }
    }
}

/// Whether every one of the substitution's EDA tool's fields' values is empty, or has no column, so it substitutes
/// nothing (`VBGUI-14`).
fn has_no_values(item: &SubstitutionsViewItem) -> bool {
    item.eda_tool
        .field_names()
        .iter()
        .all(|&field_name| {
            item.transforms
                .iter()
                .find(|transform| transform.field_name == field_name)
                .is_none_or(|transform| transform.field_value.is_empty())
        })
}

/// A changeable row's cell's background, saying whether its value is as the row needs it (`VBGUI-13`, `VBGUI-14`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Background {
    Blue,
    Red,
    Orange,
    Green,
}

impl Background {
    /// Faint, so the value's own colour can still be read, in either theme.
    fn color(self, visuals: &Visuals) -> Color32 {
        let alpha = match visuals.dark_mode {
            true => 40,
            false => 32,
        };
        let (r, g, b) = match self {
            Background::Blue => (0, 100, 255),
            Background::Red => (230, 0, 0),
            Background::Orange => (255, 140, 0),
            Background::Green => (0, 200, 0),
        };
        Color32::from_rgba_unmultiplied(r, g, b, alpha)
    }

    /// A value every row needs, red if it's empty, and green if not: a part mapping's manufacturer or MPN (`VBGUI-13`),
    /// or any of an assembly rule's values (`VBGUI-15`).
    fn of_needed_value(value: &str) -> Self {
        match value.is_empty() {
            true => Background::Red,
            false => Background::Green,
        }
    }

    /// An assembly rule's value's, its designator's, manufacturer's or MPN's, none for a column it has no value of.
    fn of_assembly_rule(item: &AssemblyRulesViewItem, column: &ColumnValue) -> Option<Self> {
        let value = match column {
            ColumnValue::RefDes => &item.rule.ref_des,
            ColumnValue::Manufacturer => &item.rule.manufacturer,
            ColumnValue::Mpn => &item.rule.mpn,
            _ => return None,
        };
        Some(Self::of_needed_value(value))
    }
}

/// What keeps a part mapping (`VBGUI-13`), a substitution (`VBGUI-14`), or an assembly rule (`VBGUI-15`), from being
/// used as it is, shown by the status column.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Problem {
    /// An assembly rule's designator is empty.
    EmptyRefDes,
    /// An assembly rule's manufacturer is empty.
    EmptyManufacturer,
    /// An assembly rule's MPN is empty.
    EmptyMpn,
    /// One of its patterns, of any field, is a regular expression that isn't valid.
    InvalidPattern,
    /// The data source has no column for one of its EDA tool's fields, a substitution's pattern's or value's.
    MissingFieldColumns(EdaTool),
    /// None of its EDA tool's fields has a pattern, empty or with no column.
    NoPatterns(EdaTool),
    /// The data source has no manufacturer's column, or no MPN's.
    MissingPartColumns,
    /// Its manufacturer, or its MPN, is empty.
    EmptyPart,
    /// None of a substitution's EDA tool's fields has a value, empty or with no column.
    NoValues,
    /// A substitution's every one of its EDA tool's fields' patterns is a value matched exactly, the same as the value
    /// it's substituted with, so it never changes a placement.
    ChangesNothing,
}

impl Problem {
    /// The row's problems, in this order, none if it can be used as it is.
    fn of(view: &TableView, row: &TableRow) -> Vec<Problem> {
        let (eda_tool, criteria) = match row {
            TableRow::PartMappings(item) => (item.eda_tool, &item.criteria),
            TableRow::Substitutions(item) => (item.eda_tool, &item.criteria),
            TableRow::AssemblyRules(item) => return Self::of_assembly_rule(item),
            // FUTURE `VBGUI-16`, the GUI's next step.
            TableRow::Parts(_) => return vec![],
        };
        let mut problems = vec![];
        if criteria.iter().any(|criterion| {
            matches!(criterion.pattern, Pattern::Regex {
                regex: Err(_),
                ..
            })
        }) {
            problems.push(Problem::InvalidPattern);
        }
        if !has_columns(view, eda_tool) {
            problems.push(Problem::MissingFieldColumns(eda_tool));
        }
        if let Some(field_name) = eda_tool.field_names().first()
            && PatternMatch::of(eda_tool, criteria, field_name) == PatternMatch::NoPatterns(eda_tool)
        {
            problems.push(Problem::NoPatterns(eda_tool));
        }
        match row {
            TableRow::PartMappings(item) => problems.extend(Self::of_part(view, item)),
            TableRow::Substitutions(item) => {
                if has_no_values(item) {
                    problems.push(Problem::NoValues);
                }
                if Self::changes_nothing(item) {
                    problems.push(Problem::ChangesNothing);
                }
            }
            TableRow::AssemblyRules(_) | TableRow::Parts(_) => {}
        }
        problems
    }

    /// An assembly rule's empty values, in the columns' order, as every rule needs all three.
    fn of_assembly_rule(item: &AssemblyRulesViewItem) -> Vec<Problem> {
        [
            (&item.rule.ref_des, Problem::EmptyRefDes),
            (&item.rule.manufacturer, Problem::EmptyManufacturer),
            (&item.rule.mpn, Problem::EmptyMpn),
        ]
        .into_iter()
        .filter(|(value, _problem)| value.is_empty())
        .map(|(_value, problem)| problem)
        .collect()
    }

    /// Whether every one of the substitution's EDA tool's fields' patterns, empty if it has none, is a value matched
    /// exactly, the same as its value, empty if it has none.
    fn changes_nothing(item: &SubstitutionsViewItem) -> bool {
        item.eda_tool
            .field_names()
            .iter()
            .all(|&field_name| {
                let pattern = item
                    .criteria
                    .iter()
                    .find(|criterion| criterion.field_name == field_name)
                    .map(|criterion| &criterion.pattern);
                let value = item
                    .transforms
                    .iter()
                    .find(|transform| transform.field_name == field_name)
                    .map_or("", |transform| transform.field_value.as_str());
                match pattern {
                    None => value.is_empty(),
                    Some(Pattern::Exact(exact)) => exact == value,
                    Some(Pattern::Regex {
                        ..
                    }) => false,
                }
            })
    }

    /// A part mapping's part's problems.
    fn of_part(view: &TableView, item: &PartMappingsViewItem) -> Option<Problem> {
        let has_column = |value: ColumnValue| {
            view.columns
                .iter()
                .any(|column| column.value == value && column.reference.is_some())
        };
        if !has_column(ColumnValue::Manufacturer) || !has_column(ColumnValue::Mpn) {
            Some(Problem::MissingPartColumns)
        } else if item.part.manufacturer.is_empty() || item.part.mpn.is_empty() {
            Some(Problem::EmptyPart)
        } else {
            None
        }
    }

    /// Said when the status is hovered.
    fn text(self) -> String {
        match self {
            Problem::EmptyRefDes => tr!("data-table-problem-empty-ref-des"),
            Problem::EmptyManufacturer => tr!("data-table-problem-empty-manufacturer"),
            Problem::EmptyMpn => tr!("data-table-problem-empty-mpn"),
            Problem::InvalidPattern => tr!("data-table-problem-invalid-pattern"),
            Problem::MissingFieldColumns(eda_tool) => {
                tr!("data-table-problem-missing-field-columns", { tool: eda_tool_name(eda_tool) })
            }
            Problem::NoPatterns(eda_tool) => {
                tr!("data-table-problem-no-patterns", { tool: eda_tool_name(eda_tool) })
            }
            Problem::MissingPartColumns => tr!("data-table-problem-missing-part-columns"),
            Problem::EmptyPart => tr!("data-table-problem-empty-part"),
            Problem::NoValues => tr!("data-table-problem-no-values"),
            Problem::ChangesNothing => tr!("data-table-problem-changes-nothing"),
        }
    }
}

/// The status column's header, a circle half filled, depicting its two states. egui's own fonts have no traffic light,
/// nor any colour emoji.
const STATUS_HEADER: &str = "◑";
/// A status, a circle tinted red or green, as egui's own fonts have no colour emoji, nor `●`.
const STATUS_GLYPH: &str = "⏺";

/// The status of a part mapping's, or a substitution's, row: a red circle if it has problems, saying what they are when it's hovered, and a
/// green one if it has none (`VBGUI-13`).
fn show_status(ui: &mut Ui, problems: &[Problem]) {
    let (color, text) = match problems.is_empty() {
        true => (
            match ui.visuals().dark_mode {
                true => Color32::LIGHT_GREEN,
                false => Color32::DARK_GREEN,
            },
            tr!("data-table-status-no-problems"),
        ),
        false => (
            ui.visuals().error_fg_color,
            problems
                .iter()
                .map(|problem| problem.text())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    };
    // a glyph, not text, so it can't be selected.
    ui.add(Label::new(RichText::new(STATUS_GLYPH).color(color)).selectable(false));
    ui.interact(ui.max_rect(), ui.id().with("hover"), Sense::hover())
        .on_hover_text(text);
}

/// A pattern, coloured by what it is, as it was parsed when it was read, not each frame. A regular expression that isn't
/// valid says why when its cell is hovered.
fn show_pattern(ui: &mut Ui, pattern: &Pattern) {
    ui.colored_label(pattern_color(pattern, ui.visuals()), pattern.text());
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
    let status = has_status(&view).then(|| {
        AxisParameters::default()
            .default_dimension(30.0)
            .resizable(false)
            .name(STATUS_HEADER.to_string())
    });
    let mut column_parameters: Vec<AxisParameters> = status
        .into_iter()
        .chain(view.columns.iter().map(|column| {
            AxisParameters::default()
                .default_dimension(150.0)
                .resizable(true)
                .name(column.name.clone())
        }))
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
    match editing.filter(|_| is_changed_here(&view)) {
        Some(editing) => {
            let send = editing.send;
            let state = editing.state;
            // once the columns a change needs are added, and shown, the change is made, of the row as it's shown now,
            // or, if they weren't added, it isn't (`STORE-21`).
            if let Some(pending) = &state.pending
                && !editing.busy
            {
                let made = (!editing.refused)
                    .then(|| pending.clone().made(&view))
                    .flatten();
                send(TableUiCommand::ColumnsShown(made));
            }
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
        AssemblyRule, AssemblyRulesViewItem, ColumnRef, ColumnValue, EdaSubstitutionRuleTransformItem, EdaTool,
        FieldPattern, Part, PartMappingsViewItem, Pattern, Revision, RowKey, RowOrdering, Source,
        SubstitutionsViewItem, Table, TableChange, TableColumn, TableKind, TableRow, TableView,
    };

    use super::{
        AfterChange, Background, CellEdit, EditedRow, PatternMatch, PendingChange, PendingRowChange, Problem,
        STATUS_GLYPH, STATUS_HEADER, SelectAfterChange, ShownAfterChange, Substitution, TableEditorState, TableRequest,
        TableState, TableUiCommand, cell_edit, new_row, pattern_color, same_row, update_table, view_column, with_edit,
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
                    tools_without_columns: vec![],
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
            Some(TableRequest::Change(TableChange::Update {
                key: RowKey("1".to_string()),
                old: row("R2", "MPN2"),
                new: row("R2", "MPN5"),
            }))
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
            tools_without_columns: vec![],
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
        assert_eq!(change, Some(TableRequest::Change(update)));
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
                    .map(|row| {
                        let TableRow::PartMappings(item) = row else {
                            unreachable!("part mappings")
                        };
                        item.clone()
                    })
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
        assert_eq!(change, Some(TableRequest::Change(add)));
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

    fn with_tool(row: TableRow, eda_tool: EdaTool) -> TableRow {
        let TableRow::PartMappings(mut item) = row else {
            unreachable!("part mappings")
        };
        item.eda_tool = eda_tool;
        TableRow::PartMappings(item)
    }

    /// `VBGUI-13`, `STORE-21`, an edit giving a row an EDA tool whose columns the table doesn't have adds them first, the
    /// change of the row's tool pending until they're shown, and one whose columns it has, or of another value, is
    /// made at once.
    #[test]
    fn an_edit_giving_a_tool_without_its_columns_adds_them_first() {
        let edited = |tools_without_columns: Vec<EdaTool>| EditedRow {
            key: RowKey("1".to_string()),
            row: row("R2", "MPN2"),
            column: ColumnValue::Eda,
            tools_without_columns,
        };

        let mut state = TableState::default();
        let command = edited(vec![EdaTool::KiCad])
            .command(CellEdit::Eda(EdaTool::KiCad))
            .unwrap();
        assert_eq!(
            update_table(&mut state, command),
            Some(TableRequest::AddColumns(EdaTool::KiCad))
        );
        assert_eq!(
            state.pending,
            Some(PendingChange {
                eda_tool: EdaTool::KiCad,
                change: PendingRowChange::Tool {
                    key: RowKey("1".to_string()),
                    row: row("R2", "MPN2"),
                },
            })
        );

        // its columns are there.
        let command = edited(vec![EdaTool::EasyEda])
            .command(CellEdit::Eda(EdaTool::KiCad))
            .unwrap();
        assert!(matches!(command, TableUiCommand::Change(TableChange::Update { .. })));
        // the row's own tool, which changes nothing.
        assert!(
            edited(vec![EdaTool::DipTrace])
                .command(CellEdit::Eda(EdaTool::DipTrace))
                .is_none()
        );
    }

    /// `VBGUI-13`, `STORE-21`, once the columns are shown, the pending change is made of the row as it's shown then, its
    /// tool changed, keeping its patterns, or not, if the row isn't as it was; a row added is added, after the others,
    /// and selected once it's shown; and nothing is made if the columns weren't added.
    #[test]
    fn a_pending_change_is_made_once_the_columns_are_shown() {
        let tool = |key: &str, row: TableRow| PendingChange {
            eda_tool: EdaTool::KiCad,
            change: PendingRowChange::Tool {
                key: RowKey(key.to_string()),
                row,
            },
        };
        // read back with an empty pattern of each field, as the columns were added.
        let mut shown = row("R2", "MPN2");
        let TableRow::PartMappings(item) = &mut shown else {
            unreachable!("part mappings")
        };
        item.criteria.push(FieldPattern {
            field_name: "package".to_string(),
            pattern: Pattern::parse(""),
        });
        let after = view("2", &[row("R1", "MPN1"), shown.clone()]);

        assert_eq!(
            tool("1", row("R2", "MPN2")).made(&after),
            Some((
                TableChange::Update {
                    key: RowKey("1".to_string()),
                    old: shown.clone(),
                    new: with_tool(shown.clone(), EdaTool::KiCad),
                },
                None
            ))
        );
        // changed since, or gone.
        assert_eq!(tool("1", row("R2", "MPN7")).made(&after), None);
        assert_eq!(tool("5", row("R2", "MPN2")).made(&after), None);

        let added = with_tool(row("", ""), EdaTool::KiCad);
        let add = PendingChange {
            eda_tool: EdaTool::KiCad,
            change: PendingRowChange::Add {
                row: added.clone(),
            },
        };
        let (change, after_change) = add.clone().made(&after).unwrap();
        assert_eq!(change, TableChange::Add {
            row: added.clone(),
        });
        assert_eq!(after_change, Some(after_change_of("2", added, 2)));

        let mut state = TableState::default();
        update_table(&mut state, TableUiCommand::ChangeAfterColumns(add.clone()));
        assert_eq!(
            update_table(&mut state, TableUiCommand::ColumnsShown(add.made(&after))),
            Some(TableRequest::Change(change))
        );
        assert_eq!(state.pending, None);
        assert!(state.after_change.is_some());

        let mut state = TableState::default();
        update_table(
            &mut state,
            TableUiCommand::ChangeAfterColumns(tool("1", row("R2", "MPN2"))),
        );
        assert_eq!(update_table(&mut state, TableUiCommand::ColumnsShown(None)), None);
        assert_eq!(state.pending, None);
        assert_eq!(state.after_change, None);
    }

    fn after_change_of(revision: &str, row: TableRow, place: usize) -> AfterChange {
        after_change(revision, select_row(row, place))
    }

    /// A part mapping of the EDA tool, with the fields' patterns.
    fn mapping_with(eda_tool: EdaTool, patterns: &[(&str, &str)]) -> PartMappingsViewItem {
        PartMappingsViewItem {
            eda_tool,
            criteria: patterns
                .iter()
                .map(|(field_name, pattern)| FieldPattern {
                    field_name: field_name.to_string(),
                    pattern: Pattern::parse(pattern),
                })
                .collect(),
            part: Part::new("MFR1".to_string(), "MPN1".to_string()),
        }
    }

    fn pattern_match(item: &PartMappingsViewItem, field_name: &str) -> PatternMatch {
        PatternMatch::of(item.eda_tool, &item.criteria, field_name)
    }

    /// `VBGUI-13`, a pattern's cell says how the row's EDA tool uses it, the first that's so: blue if the tool doesn't
    /// match its field, whatever its pattern, red if it does and its pattern is a regular expression that isn't valid,
    /// orange if it does and none of the tool's fields has a pattern in the row, empty or with no column, and green
    /// otherwise, a field shared by tools matched by each.
    #[test]
    fn a_patterns_cell_says_how_the_rows_tool_uses_it() {
        use EdaTool::*;
        use PatternMatch::*;

        let dip_trace = mapping_with(DipTrace, &[
            ("name", "RES_0402"),
            ("value", ""),
            ("package", "/R_[/"),
            ("val", ""),
        ]);
        assert_eq!(pattern_match(&dip_trace, "name"), Matched(DipTrace));
        // empty, but another of the tool's fields has a pattern.
        assert_eq!(pattern_match(&dip_trace, "value"), Matched(DipTrace));
        // not the tool's, even if it isn't valid, or is empty, or has no column.
        assert_eq!(pattern_match(&dip_trace, "package"), NotMatched(DipTrace));
        assert_eq!(pattern_match(&dip_trace, "val"), NotMatched(DipTrace));
        assert_eq!(pattern_match(&dip_trace, "device"), NotMatched(DipTrace));

        let invalid = mapping_with(EasyEda, &[("device", ""), ("value", "/10[/")]);
        assert_eq!(pattern_match(&invalid, "value"), Invalid(EasyEda));
        // the other field, empty, as the invalid one is a pattern, if not a valid one.
        assert_eq!(pattern_match(&invalid, "device"), Matched(EasyEda));

        // every one of KiCad's empty, or with no column, `val`, whatever the other tools' fields have.
        let none = mapping_with(KiCad, &[("name", "RES_0402"), ("package", "")]);
        assert_eq!(pattern_match(&none, "package"), NoPatterns(KiCad));
        assert_eq!(pattern_match(&none, "val"), NoPatterns(KiCad));
        assert_eq!(pattern_match(&none, "name"), NotMatched(KiCad));
    }

    /// `VBGUI-13`, each of a pattern cell's backgrounds is faint, and its colour, blue, red, orange or green, in either
    /// theme.
    #[test]
    fn a_patterns_cells_backgrounds_are_faint_and_told_apart() {
        for visuals in [Visuals::dark(), Visuals::light()] {
            let colour = |matched: PatternMatch| matched.background().color(&visuals);
            let blue = colour(PatternMatch::NotMatched(EdaTool::KiCad));
            let red = colour(PatternMatch::Invalid(EdaTool::KiCad));
            let orange = colour(PatternMatch::NoPatterns(EdaTool::KiCad));
            let green = colour(PatternMatch::Matched(EdaTool::KiCad));

            for background in [blue, red, orange, green] {
                assert!(background.a() < 64, "faint: {background:?}");
            }
            assert!(blue.b() > blue.r() && blue.b() > blue.g(), "blue: {blue:?}");
            assert!(red.r() > red.g() && red.r() > red.b() && red.g() == 0, "red: {red:?}");
            assert!(
                orange.r() > orange.g() && orange.g() > orange.b() && orange.g() > 0,
                "orange: {orange:?}"
            );
            assert!(green.g() > green.r() && green.g() > green.b(), "green: {green:?}");
        }
    }

    /// `VBGUI-13`, a manufacturer's, or an MPN's, cell is red if it's empty, and green if not, the same colours as a
    /// pattern's.
    #[test]
    fn a_parts_cells_are_red_if_empty_and_green_if_not() {
        assert_eq!(Background::of_needed_value(""), Background::Red);
        assert_eq!(Background::of_needed_value("MFR1"), Background::Green);
        assert_eq!(Background::Red, PatternMatch::Invalid(EdaTool::KiCad).background());
        assert_eq!(Background::Green, PatternMatch::Matched(EdaTool::KiCad).background());
    }

    /// Part mappings' columns, each with a reference, as a file's are, with a pattern's for each of the fields.
    fn columns_with(field_names: &[&str], part: bool) -> Vec<TableColumn> {
        TableColumn::part_mappings(field_names.iter().copied())
            .into_iter()
            .filter(|column| part || !matches!(column.value, ColumnValue::Manufacturer | ColumnValue::Mpn))
            .map(|column| TableColumn {
                reference: Some(ColumnRef(column.name.clone())),
                ..column
            })
            .collect()
    }

    fn view_with(columns: Vec<TableColumn>, item: &PartMappingsViewItem) -> TableView {
        TableView {
            columns,
            ..view("1", &[TableRow::PartMappings(item.clone())])
        }
    }

    fn mapping_problems(view: &TableView, item: &PartMappingsViewItem) -> Vec<Problem> {
        Problem::of(view, &TableRow::PartMappings(item.clone()))
    }

    /// `VBGUI-13`, a part mapping's problems are each found, in order: a pattern of any field that isn't valid, a
    /// column of one of its EDA tool's fields missing, none of the tool's fields with a pattern, the manufacturer's or
    /// the MPN's column missing, and, if they're there, either empty, and none if it can be used as it is.
    #[test]
    fn a_part_mappings_problems_are_each_found() {
        use EdaTool::*;
        use Problem::*;
        let all = ["name", "value", "package", "val", "device"];

        let good = mapping_with(DipTrace, &[("name", "RES_0402"), ("value", "")]);
        assert_eq!(
            mapping_problems(&view_with(columns_with(&all, true), &good), &good),
            vec![]
        );

        // KiCad's `val` has no column, and neither of its fields has a pattern, and another tool's isn't valid.
        let bad = mapping_with(KiCad, &[("name", "/R_[/"), ("package", "")]);
        let columns = columns_with(&["name", "value", "package"], true);
        assert_eq!(mapping_problems(&view_with(columns, &bad), &bad), vec![
            InvalidPattern,
            MissingFieldColumns(KiCad),
            NoPatterns(KiCad),
        ]);

        // the part's columns missing, so whether it's empty isn't said.
        let mut empty = good.clone();
        empty.part = Part::new(String::new(), "MPN1".to_string());
        assert_eq!(
            mapping_problems(&view_with(columns_with(&all, false), &empty), &empty),
            vec![MissingPartColumns]
        );
        assert_eq!(
            mapping_problems(&view_with(columns_with(&all, true), &empty), &empty),
            vec![EmptyPart]
        );
        empty.part = Part::new("MFR1".to_string(), String::new());
        assert_eq!(
            mapping_problems(&view_with(columns_with(&all, true), &empty), &empty),
            vec![EmptyPart]
        );
    }

    /// A substitution of the EDA tool, with the fields' patterns and values, and the comment.
    fn substitution_with(
        eda_tool: EdaTool,
        patterns: &[(&str, &str)],
        values: &[(&str, &str)],
        comment: Option<&str>,
    ) -> SubstitutionsViewItem {
        SubstitutionsViewItem {
            eda_tool,
            criteria: patterns
                .iter()
                .map(|(field_name, pattern)| FieldPattern {
                    field_name: field_name.to_string(),
                    pattern: Pattern::parse(pattern),
                })
                .collect(),
            transforms: values
                .iter()
                .map(|(field_name, value)| EdaSubstitutionRuleTransformItem {
                    field_name: field_name.to_string(),
                    field_value: value.to_string(),
                })
                .collect(),
            comment: comment.map(str::to_string),
        }
    }

    /// Substitutions' columns, each with a reference, as a file's are, a pattern's and a value's for each of the
    /// fields.
    fn substitution_columns(pattern_fields: &[&str], value_fields: &[&str], comment: bool) -> Vec<TableColumn> {
        TableColumn::substitutions(pattern_fields, value_fields, comment)
            .into_iter()
            .map(|column| TableColumn {
                reference: Some(ColumnRef(column.name.clone())),
                ..column
            })
            .collect()
    }

    fn substitutions_view(columns: &[TableColumn], items: &[SubstitutionsViewItem]) -> TableView {
        TableView {
            kind: TableKind::Substitutions,
            columns: columns.to_vec(),
            table: Table::Substitutions(items.to_vec()),
            keys: (0..items.len())
                .map(|index| RowKey(index.to_string()))
                .collect(),
            ..view("1", &[])
        }
    }

    fn substitution_problems(columns: &[TableColumn], item: &SubstitutionsViewItem) -> Vec<Problem> {
        let view = substitutions_view(columns, std::slice::from_ref(item));
        Problem::of(&view, &TableRow::Substitutions(item.clone()))
    }

    /// `VBGUI-14`, a substitution's problems are each found, in order: a pattern of any field that isn't valid, a
    /// pattern's or a value's column of one of its EDA tool's fields missing, none of the tool's fields with a pattern,
    /// none with a value, and every one of the tool's patterns a value matched exactly, the same as its value, and none
    /// if it has none.
    #[test]
    fn a_substitutions_problems_are_each_found() {
        use EdaTool::*;
        use Problem::*;
        let all = ["name", "value", "package", "val", "device"];
        let columns = substitution_columns(&all, &all, true);

        let good = substitution_with(
            DipTrace,
            &[("name", "R0402"), ("value", "")],
            &[("name", "RES_0402"), ("value", "")],
            Some(""),
        );
        assert_eq!(substitution_problems(&columns, &good), vec![]);
        // a regular expression is never the value it's substituted with.
        let regex = substitution_with(DipTrace, &[("name", "/R0402/")], &[("name", "/R0402/")], None);
        assert_eq!(substitution_problems(&columns, &regex), vec![]);

        // another tool's pattern isn't valid, and KiCad's `val` has no value's column, and neither of its fields has a
        // pattern, nor a value, so it changes nothing, as each is substituted with nothing.
        let bad = substitution_with(KiCad, &[("name", "/R_[/"), ("package", "")], &[("package", "")], None);
        let missing_value = substitution_columns(&all, &["name", "value", "package"], false);
        assert_eq!(substitution_problems(&missing_value, &bad), vec![
            InvalidPattern,
            MissingFieldColumns(KiCad),
            NoPatterns(KiCad),
            NoValues,
            ChangesNothing,
        ]);
        // patterns, but every one of the tool's values empty, whatever the other tools' fields have.
        let no_values = substitution_with(
            DipTrace,
            &[("name", "R0402")],
            &[("name", ""), ("value", ""), ("package", "R_0402")],
            None,
        );
        assert_eq!(substitution_problems(&columns, &no_values), vec![NoValues]);
        // a pattern's column missing, too.
        let missing_pattern = substitution_columns(&["name", "value", "package"], &all, false);
        assert!(substitution_problems(&missing_pattern, &bad).contains(&MissingFieldColumns(KiCad)));

        // each of the tool's patterns the value it's substituted with.
        let same = substitution_with(
            DipTrace,
            &[("name", "R0402"), ("value", "10K")],
            &[("name", "R0402"), ("value", "10K")],
            None,
        );
        assert_eq!(substitution_problems(&columns, &same), vec![ChangesNothing]);
        // only the tool's, the other tools' fields being ignored.
        let other = substitution_with(
            DipTrace,
            &[("name", "R0402"), ("package", "R_0402")],
            &[("name", "RES_0402"), ("package", "R_0402")],
            None,
        );
        assert_eq!(substitution_problems(&columns, &other), vec![]);
    }

    /// `VBGUI-14`, a substitution's value's cell is blue if the row's tool doesn't substitute its field, red if it does
    /// and every one of the tool's values is empty, or has no column, and green otherwise, an empty value too, as a
    /// field can be substituted with nothing.
    #[test]
    fn a_values_cell_says_whether_the_rows_tool_substitutes_it() {
        use EdaTool::*;
        use Substitution::*;

        let dip_trace = substitution_with(DipTrace, &[], &[("name", "RES_0402"), ("value", "")], None);
        assert_eq!(Substitution::of(&dip_trace, "name"), Substituted(DipTrace));
        assert_eq!(Substitution::of(&dip_trace, "value"), Substituted(DipTrace));
        assert_eq!(Substitution::of(&dip_trace, "package"), NotSubstituted(DipTrace));

        // `val` has no value, and `package` an empty one, so both are red, whatever the other tools' fields have.
        let kicad = substitution_with(KiCad, &[], &[("name", "RES_0402"), ("package", "")], None);
        assert_eq!(Substitution::of(&kicad, "package"), NoValues(KiCad));
        assert_eq!(Substitution::of(&kicad, "val"), NoValues(KiCad));
        assert_eq!(Substitution::of(&kicad, "name"), NotSubstituted(KiCad));

        assert_eq!(Substituted(KiCad).background(), Background::Green);
        assert_eq!(NoValues(KiCad).background(), Background::Red);
        assert_eq!(NotSubstituted(KiCad).background(), Background::Blue);
    }

    /// `VBGUI-14`, a substitution's pattern, value and comment are edited as a part mapping's pattern is, a value or a
    /// comment left empty not being one the row didn't have, so it's the same row as it's read back, with every column's
    /// value.
    #[test]
    fn a_substitutions_cells_are_edited() {
        let row = TableRow::Substitutions(substitution_with(
            EdaTool::DipTrace,
            &[("name", "R0402")],
            &[("name", "RES_0402")],
            None,
        ));
        let value = ColumnValue::Substitute("name".to_string());
        assert_eq!(cell_edit(&row, &value), Some(CellEdit::Text("RES_0402".to_string())));
        assert_eq!(
            cell_edit(&row, &ColumnValue::Comment),
            Some(CellEdit::Text(String::new()))
        );

        let edited = with_edit(&row, &value, CellEdit::Text("RES_0603".to_string()));
        let edited = with_edit(&edited, &ColumnValue::Comment, CellEdit::Text("comment 1".to_string()));
        let edited = with_edit(
            &edited,
            &ColumnValue::Pattern("value".to_string()),
            CellEdit::Pattern(Pattern::parse("/10.*/")),
        );
        assert_eq!(
            edited,
            TableRow::Substitutions(substitution_with(
                EdaTool::DipTrace,
                &[("name", "R0402"), ("value", "/10.*/")],
                &[("name", "RES_0603")],
                Some("comment 1"),
            ))
        );

        // left empty, so unchanged.
        assert_eq!(
            with_edit(
                &row,
                &ColumnValue::Substitute("value".to_string()),
                CellEdit::Text(String::new())
            ),
            row
        );
        assert_eq!(
            with_edit(&row, &ColumnValue::Comment, CellEdit::Text(String::new())),
            row
        );

        // as it's read back, with every column's value, empty if it had none.
        let read = TableRow::Substitutions(substitution_with(
            EdaTool::DipTrace,
            &[("name", "R0402"), ("value", "")],
            &[("name", "RES_0402"), ("value", "")],
            Some(""),
        ));
        assert!(same_row(&row, &read));
        assert!(!same_row(&row, &edited));
    }

    /// `VBGUI-13`, `VBGUI-14`, `VBGUI-15`, the status column is first, so a part mappings', substitutions' or assembly
    /// rules' table's columns are the view's after it, and other tables have none.
    #[test]
    fn the_status_column_is_first_of_changeable_tables_only() {
        let item = mapping_with(EdaTool::DipTrace, &[("name", "RES_0402")]);
        let mappings = view_with(columns_with(&["name"], true), &item);
        assert!(view_column(&mappings, 0).is_none());
        assert_eq!(view_column(&mappings, 1), mappings.columns.first());
        assert_eq!(view_column(&mappings, 4), mappings.columns.get(3));
        assert!(view_column(&mappings, 5).is_none());

        let substitutions = substitutions_view(&substitution_columns(&["name"], &["name"], true), &[]);
        assert!(view_column(&substitutions, 0).is_none());
        assert_eq!(view_column(&substitutions, 1), substitutions.columns.first());

        let rules = assembly_rules_view(&[]);
        assert!(view_column(&rules, 0).is_none());
        assert_eq!(view_column(&rules, 1), rules.columns.first());
        assert_eq!(view_column(&rules, 3), rules.columns.get(2));
        assert!(view_column(&rules, 4).is_none());

        let bom = TableView {
            table: Table::Bom(vec![]),
            ..mappings.clone()
        };
        assert_eq!(view_column(&bom, 0), bom.columns.first());
    }

    fn assembly_rule(ref_des: &str, manufacturer: &str, mpn: &str) -> AssemblyRulesViewItem {
        AssemblyRulesViewItem {
            rule: AssemblyRule {
                ref_des: ref_des.to_string(),
                manufacturer: manufacturer.to_string(),
                mpn: mpn.to_string(),
            },
        }
    }

    /// Assembly rules, their columns each with a reference, as a file's are.
    fn assembly_rules_view(items: &[AssemblyRulesViewItem]) -> TableView {
        let table = Table::AssemblyRules(items.to_vec());
        TableView {
            kind: TableKind::AssemblyRules,
            columns: table
                .columns()
                .into_iter()
                .map(|column| TableColumn {
                    reference: Some(ColumnRef(column.name.clone())),
                    ..column
                })
                .collect(),
            keys: (0..items.len())
                .map(|index| RowKey(index.to_string()))
                .collect(),
            table,
            ..view("1", &[])
        }
    }

    fn rule_problems(item: &AssemblyRulesViewItem) -> Vec<Problem> {
        let view = assembly_rules_view(std::slice::from_ref(item));
        Problem::of(&view, &TableRow::AssemblyRules(item.clone()))
    }

    /// `VBGUI-15`, an assembly rule's problems are each of its values that's empty, in the columns' order, and none if
    /// it has none.
    #[test]
    fn an_assembly_rules_problems_are_each_found() {
        use Problem::*;

        assert_eq!(rule_problems(&assembly_rule("R1", "MFR1", "MPN1")), vec![]);
        assert_eq!(rule_problems(&assembly_rule("R1", "MFR1", "")), vec![EmptyMpn]);
        assert_eq!(rule_problems(&assembly_rule("", "MFR1", "")), vec![
            EmptyRefDes,
            EmptyMpn
        ]);
        assert_eq!(rule_problems(&assembly_rule("", "", "")), vec![
            EmptyRefDes,
            EmptyManufacturer,
            EmptyMpn
        ]);
    }

    /// `VBGUI-15`, each of an assembly rule's cells is red if it's empty, and green if not.
    #[test]
    fn an_assembly_rules_cells_are_red_if_empty_and_green_if_not() {
        let item = assembly_rule("R1", "", "MPN1");
        assert_eq!(
            Background::of_assembly_rule(&item, &ColumnValue::RefDes),
            Some(Background::Green)
        );
        assert_eq!(
            Background::of_assembly_rule(&item, &ColumnValue::Manufacturer),
            Some(Background::Red)
        );
        assert_eq!(
            Background::of_assembly_rule(&item, &ColumnValue::Mpn),
            Some(Background::Green)
        );
        assert_eq!(Background::of_assembly_rule(&item, &ColumnValue::Eda), None);
    }

    /// `VBGUI-15`, an assembly rule's values are edited as text, and a new row is an empty rule, whether or not there's
    /// an EDA tool, as a rule has none, though a part mapping, or a substitution, needs one.
    #[test]
    fn an_assembly_rules_cells_are_edited_and_a_new_one_is_empty() {
        let row = TableRow::AssemblyRules(assembly_rule("R1", "MFR1", "MPN1"));
        assert_eq!(
            cell_edit(&row, &ColumnValue::RefDes),
            Some(CellEdit::Text("R1".to_string()))
        );
        let edited = with_edit(&row, &ColumnValue::RefDes, CellEdit::Text("R2".to_string()));
        let edited = with_edit(&edited, &ColumnValue::Manufacturer, CellEdit::Text("MFR2".to_string()));
        let edited = with_edit(&edited, &ColumnValue::Mpn, CellEdit::Text(String::new()));
        assert_eq!(edited, TableRow::AssemblyRules(assembly_rule("R2", "MFR2", "")));

        let empty = Some(TableRow::AssemblyRules(assembly_rule("", "", "")));
        assert_eq!(new_row(TableKind::AssemblyRules, None), empty);
        assert_eq!(new_row(TableKind::AssemblyRules, Some(EdaTool::KiCad)), empty);
        assert_eq!(new_row(TableKind::PartMappings, None), None);
        assert_eq!(new_row(TableKind::Substitutions, None), None);
    }

    /// `VBGUI-13`, the status column's header and its circle are glyphs egui's own fonts have, so they're not shown as
    /// boxes.
    #[test]
    fn the_status_glyphs_are_in_the_default_fonts() {
        let ctx = egui::Context::default();
        // fonts are built at the start of a frame.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        for glyph in [STATUS_HEADER, STATUS_GLYPH] {
            assert!(
                ctx.fonts_mut(|fonts| fonts.has_glyphs(&egui::FontId::proportional(14.0), glyph)),
                "{glyph} has no glyph"
            );
        }
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
