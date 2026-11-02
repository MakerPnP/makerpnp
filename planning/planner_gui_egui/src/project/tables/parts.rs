use std::collections::BTreeSet;
use std::fmt::Display;

use derivative::Derivative;
use egui::Ui;
use egui_deferred_table::{
    Action, ApplyChange, AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource,
    DeferredTableRenderer, EditableTableRenderer, EditorState, TableDimensions, apply_reordering,
};
use egui_i18n::tr;
use planner_app::{Part, PartStates, PartWithState, ProcessReference, RefDes};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::sync::Enqueue;
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

mod columns {
    pub const MANUFACTURER_COL: usize = 0;
    pub const MPN_COL: usize = 1;
    pub const QUANTITY_COL: usize = 2;
    pub const PROCESSES_COL: usize = 3;
    pub const REF_DES_SET_COL: usize = 4;

    /// count of columns
    pub const COLUMN_COUNT: usize = 5;
}
use columns::*;

#[derive(Debug)]
pub struct PartDataSource {
    rows: Vec<PartWithState>,

    processes: Vec<ProcessReference>,
}

#[derive(Debug)]
pub struct PartRenderer {
    rows_to_filter: Vec<usize>,
    row_ordering: Option<Vec<usize>>,
    column_ordering: Option<Vec<usize>>,
}

#[derive(Debug)]
pub struct PartEditor {
    sender: Enqueue<PartTableUiCommand>,
}

#[derive(Debug, Clone)]
pub enum PartCellEditState {
    Processes(Vec<(ProcessReference, bool)>),
}

enum PartCellEditStateError {
    #[allow(dead_code)]
    None,
}

impl ApplyChange<PartCellEditState, PartCellEditStateError> for PartWithState {
    fn apply_change(&mut self, value: PartCellEditState) -> Result<(), PartCellEditStateError> {
        match value {
            PartCellEditState::Processes(value) => {
                self.processes = value
                    .into_iter()
                    .filter_map(|(process, enabled)| if enabled { Some(process.clone()) } else { None })
                    .collect();

                Ok(())
            }
        }
    }
}

impl PartDataSource {
    pub fn new(mut processes: Vec<ProcessReference>) -> Self {
        // sorting the processes here helps to ensure that the view vs edit list of processes has the same
        // ordering.
        processes.sort();

        Self {
            processes,
            rows: Default::default(),
        }
    }

    pub fn update_parts(&mut self, mut part_states: PartStates) {
        self.rows = part_states.parts.drain(..).collect();
    }
}

impl PartRenderer {
    pub fn new() -> Self {
        Self {
            rows_to_filter: Default::default(),
            row_ordering: None,
            column_ordering: None,
        }
    }
}

impl PartEditor {
    pub fn new(sender: Enqueue<PartTableUiCommand>) -> Self {
        Self {
            sender,
        }
    }
}

impl EditableTableRenderer<PartDataSource> for PartEditor {
    type Value = PartWithState;
    type ItemState = PartCellEditState;

    fn build_item_state(
        &self,
        cell_index: CellIndex,
        source: &mut PartDataSource,
    ) -> Option<(PartCellEditState, PartWithState)> {
        let original_item = &source.rows[cell_index.row];

        match cell_index.column {
            PROCESSES_COL => {
                let enabled_processes = source
                    .processes
                    .iter()
                    .map(|process| {
                        (
                            process.clone(),
                            original_item
                                .processes
                                .contains(process),
                        )
                    })
                    .collect::<Vec<(ProcessReference, bool)>>();

                Some((PartCellEditState::Processes(enabled_processes), original_item.clone()))
            }
            _ => None,
        }
    }

    fn on_edit_complete(
        &mut self,
        index: CellIndex,
        state: PartCellEditState,
        original_item: PartWithState,
        source: &mut PartDataSource,
    ) {
        let _ = source;

        self.sender
            .send(PartTableUiCommand::CellEditComplete(index, state, original_item))
            .expect("sent");
    }

    fn render_cell_editor(
        &self,
        ui: &mut Ui,
        cell_index: &CellIndex,
        state: &mut Self::ItemState,
        _original_item: &Self::Value,
        _source: &mut PartDataSource,
    ) {
        match state {
            PartCellEditState::Processes(enabled_processes) => {
                let mut enabled_processes_mut = enabled_processes.clone();
                let _response = ui.add(|ui: &mut Ui| {
                    // FIXME this doesn't always fit in the available space, what to do?
                    ui.horizontal(|ui| {
                        // Note that the enabled_processes was built in the same order as self.processes.
                        for (name, enabled) in enabled_processes_mut.iter_mut() {
                            ui.checkbox(enabled, name.to_string());
                        }
                    })
                    .response
                });

                if enabled_processes_mut != *enabled_processes {
                    // NOTE: if we had &mut self here, we could apply the edit state now
                    self.sender
                        .send(PartTableUiCommand::ApplyCellEdit {
                            edit: PartCellEditState::Processes(enabled_processes_mut),
                            cell_index: *cell_index,
                        })
                        .expect("sent");
                }
            }
        }
    }
}

impl DeferredTableDataSource for PartDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: COLUMN_COUNT,
        }
    }
}

impl DeferredTableRenderer<PartDataSource> for PartRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &PartDataSource) {
        let row = &source.rows[cell_index.row];

        let _ = match cell_index.column {
            MANUFACTURER_COL => ui.label(&row.part.manufacturer),
            MPN_COL => ui.label(&row.part.mpn),
            PROCESSES_COL => {
                // Build in the same order as self.processes, specifically not just iterating over `row.processes`
                let processes = source
                    .processes
                    .iter()
                    .filter(|process| row.processes.contains(process))
                    .map(|process| process.to_string())
                    .collect::<Vec<String>>();

                let processes_label: String = processes.join(", ");
                ui.label(processes_label)
            }
            REF_DES_SET_COL => {
                let label: String = refdes_set_to_string(&row.ref_des_set);
                ui.label(label)
            }
            QUANTITY_COL => ui.label(format!("{}", row.quantity)),
            _ => unreachable!(),
        };
    }

    fn rows_to_filter(&self) -> Option<&[usize]> {
        Some(self.rows_to_filter.as_slice())
    }

    fn row_ordering(&self) -> Option<&[usize]> {
        self.row_ordering
            .as_ref()
            .map(|v| v.as_slice())
    }

    fn column_ordering(&self) -> Option<&[usize]> {
        self.column_ordering
            .as_ref()
            .map(|v| v.as_slice())
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PartTableUi {
    source: Value<(
        PartDataSource,
        PartRenderer,
        PartEditor,
        EditorState<PartCellEditState, PartWithState>,
    )>,
    #[derivative(Debug = "ignore")]
    filter: Filter,

    pub component: ComponentState<PartTableUiCommand>,
}

impl PartTableUi {
    pub fn new(processes: Vec<ProcessReference>) -> Self {
        let component = ComponentState::default();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(component.sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                PartTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            source: Value::new((
                PartDataSource::new(processes),
                PartRenderer::new(),
                PartEditor::new(component.sender.clone()),
                EditorState::default(),
            )),
            filter,
            component,
        }
    }

    pub fn update_processes(&mut self, processes: Vec<ProcessReference>) {
        let (source, _renderer, _editor, _editor_state) = &mut *self.source.lock().unwrap();

        source.processes = processes;
    }

    pub fn update_parts(&mut self, part_states: PartStates) {
        let (source, _renderer, _editor, _editor_state) = &mut *self.source.lock().unwrap();

        source.update_parts(part_states);
    }

    pub fn filter_ui(&self, ui: &mut Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum PartTableUiCommand {
    None,
    FilterCommand(FilterUiCommand),
    ApplyCellEdit {
        edit: PartCellEditState,
        cell_index: CellIndex,
    },
    CellEditComplete(CellIndex, PartCellEditState, PartWithState),
    NewSelection(Vec<Part>),
}

#[derive(Debug, Clone)]
pub enum PartTableUiAction {
    None,
    RequestRepaint,
    ItemUpdated {
        index: CellIndex,
        item: PartWithState,
        original_item: PartWithState,
    },
    ApplySelection(Vec<Part>),
}

#[derive(Debug, Clone, Default)]
pub struct PartTableUiContext {}

impl UiComponent for PartTableUi {
    type UiContext<'context> = PartTableUiContext;
    type UiCommand = PartTableUiCommand;
    type UiAction = PartTableUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let (source, renderer, editor, editor_state) = &mut *self.source.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("parts_table"))
            .min_size((400.0, 400.0).into())
            .column_parameters(&vec![
                AxisParameters::default()
                    .name(tr!("table-parts-column-manufacturer"))
                    .default_dimension(200.0),
                AxisParameters::default()
                    .name(tr!("table-parts-column-mpn"))
                    .default_dimension(200.0),
                AxisParameters::default()
                    .name(tr!("table-parts-column-quantity"))
                    .default_dimension(100.0),
                AxisParameters::default()
                    .name(tr!("table-parts-column-processes"))
                    .default_dimension(100.0),
                AxisParameters::default()
                    .name(tr!("table-parts-column-ref-des-set"))
                    .expandable(true)
                    .default_dimension(100.0),
            ])
            .show_and_edit(ui, source, renderer, editor, editor_state);

        for action in actions {
            match action {
                // TODO we need double-click to edit cells, not single-click, then single-click again
                Action::CellClicked(cell_index) => {
                    info!("Cell clicked. cell: {:?}", cell_index);
                }
                Action::ColumnReorder {
                    from,
                    to,
                } => {
                    apply_reordering(&mut renderer.column_ordering, from, to);
                }
                Action::RowReorder {
                    from,
                    to,
                } => {
                    apply_reordering(&mut renderer.column_ordering, from, to);
                }
                Action::RowSelectionChanged {
                    selection,
                } => {
                    let parts = selection
                        .iter()
                        .map(|row| source.rows[*row].part.clone())
                        .collect::<Vec<_>>();
                    editor
                        .sender
                        .send(PartTableUiCommand::NewSelection(parts))
                        .expect("sent");
                }
            }
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PartTableUiCommand::None => Some(PartTableUiAction::None),
            PartTableUiCommand::FilterCommand(command) => {
                let action = self
                    .filter
                    .update(command, &mut FilterUiContext::default())
                    .inspect(|action| debug!("filter action: {:?}", action));

                match action {
                    Some(FilterUiAction::ApplyFilter) => {
                        let (source, renderer, _editor, _editor_state) = &mut *self.source.lock().unwrap();

                        renderer.rows_to_filter = source
                            .rows
                            .iter()
                            .enumerate()
                            .filter_map(|(id, row)| {
                                let processes: String = format_string_array(&row.processes);
                                let ref_des_set: String = format_string_array(&row.ref_des_set);

                                let haystack = format!(
                                    "manufacturer: '{}', mpn: '{}', processes: {}, ref_des_set: {}",
                                    row.part.manufacturer, row.part.mpn, processes, ref_des_set,
                                );

                                // "Filter single row. If this returns false, the row will be hidden."
                                let result = self.filter.matches(haystack.as_str());

                                trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);
                                if !result { Some(id) } else { None }
                            })
                            .collect::<Vec<usize>>();

                        Some(PartTableUiAction::RequestRepaint)
                    }
                    None => None,
                }
            }

            PartTableUiCommand::ApplyCellEdit {
                edit: new_edit_state,
                cell_index,
            } => {
                let (_source, _renderer, _editor, editor_state) = &mut *self.source.lock().unwrap();
                match editor_state.state.as_mut() {
                    Some(CellEditState::Editing(current_cell_index, current_edit_state, _original_item))
                        if *current_cell_index == cell_index =>
                    {
                        debug!("edit state changed. cell: {:?}, edit: {:?}", cell_index, new_edit_state);
                        *current_edit_state = new_edit_state;
                    }
                    _ => {}
                }
                None
            }
            PartTableUiCommand::CellEditComplete(cell_index, edit_state, original_item) => {
                let (source, _renderer, _editor, _editor_state) = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                row.apply_change(edit_state)
                    .map(|_| PartTableUiAction::ItemUpdated {
                        index: cell_index,
                        item: row.clone(),
                        original_item,
                    })
                    .ok()
            }
            PartTableUiCommand::NewSelection(parts) => Some(PartTableUiAction::ApplySelection(parts)),
        }
    }
}

fn refdes_set_to_string(ref_des_set: &BTreeSet<RefDes>) -> String {
    ref_des_set
        .iter()
        .cloned()
        .map(|meh| meh.to_string())
        .collect::<Vec<String>>()
        .join(", ")
}

fn format_string_array<T, I>(items: I) -> String
where
    T: Display,
    I: IntoIterator<Item = T>,
{
    format!(
        "[{}]",
        items
            .into_iter()
            .map(|it| format!("'{}'", it))
            .collect::<Vec<String>>()
            .join(", ")
    )
}
