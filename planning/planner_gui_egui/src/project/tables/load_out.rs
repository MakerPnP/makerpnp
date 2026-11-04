use derivative::Derivative;
use egui::{Color32, CornerRadius, Stroke, StrokeKind, Ui};
use egui_deferred_table::{
    Action, ApplyChange, AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource,
    DeferredTableRenderer, EditableTableRenderer, EditorState, TableDimensions, apply_reordering,
};
use egui_i18n::tr;
use planner_app::{LoadOut, LoadOutItem, Reference};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::sync::Enqueue;
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

mod columns {
    pub const FEEDER_REFERENCE_COL: usize = 0;
    pub const MANUFACTURER_COL: usize = 1;
    pub const MPN_COL: usize = 2;

    /// count of columns
    pub const COLUMN_COUNT: usize = 3;
}
use columns::*;

#[derive(Debug, Clone)]
pub struct LoadOutDataSource {
    rows: Vec<LoadOutItem>,
}

#[derive(Debug)]
struct LoadOutRenderer {
    rows_to_filter: Vec<usize>,
    row_ordering: Option<Vec<usize>>,
    column_ordering: Option<Vec<usize>>,
}

#[derive(Debug)]
struct LoadOutEditor {
    sender: Enqueue<LoadOutTableUiCommand>,
}

#[derive(Debug, Clone)]
pub enum LoadoutItemCellEditState {
    FeederReference(String),
}

enum LoadoutItemCellEditStateError {
    InvalidFeederReference,
}

impl ApplyChange<LoadoutItemCellEditState, LoadoutItemCellEditStateError> for LoadOutItem {
    fn apply_change(&mut self, value: LoadoutItemCellEditState) -> Result<(), LoadoutItemCellEditStateError> {
        match value {
            LoadoutItemCellEditState::FeederReference(value) => {
                if value.is_empty() {
                    self.reference = None;
                    Ok(())
                } else {
                    Reference::try_from(value)
                        .map(|reference| {
                            self.reference = Some(reference);
                        })
                        .map_err(|_| LoadoutItemCellEditStateError::InvalidFeederReference)
                }
            }
        }
    }
}

impl LoadOutDataSource {
    pub fn new() -> Self {
        Self {
            rows: Default::default(),
        }
    }

    pub fn update_loadout(&mut self, mut load_out: LoadOut) {
        self.rows = load_out.items.drain(..).collect();
    }
}

impl LoadOutRenderer {
    pub fn new() -> Self {
        Self {
            rows_to_filter: Default::default(),
            row_ordering: None,
            column_ordering: None,
        }
    }
}

impl LoadOutEditor {
    pub fn new(sender: Enqueue<LoadOutTableUiCommand>) -> Self {
        Self {
            sender,
        }
    }
}

impl EditableTableRenderer<LoadOutDataSource> for LoadOutEditor {
    type Value = LoadOutItem;
    type ItemState = LoadoutItemCellEditState;

    fn build_item_state(
        &self,
        cell_index: CellIndex,
        source: &mut LoadOutDataSource,
    ) -> Option<(LoadoutItemCellEditState, LoadOutItem)> {
        let original_item = &source.rows[cell_index.row];

        match cell_index.column {
            FEEDER_REFERENCE_COL => Some((
                LoadoutItemCellEditState::FeederReference(
                    original_item
                        .reference
                        .as_ref()
                        .map_or("".to_string(), |value| value.to_string()),
                ),
                original_item.clone(),
            )),
            _ => None,
        }
    }

    fn on_edit_complete(
        &mut self,
        cell_index: CellIndex,
        edit_state: Self::ItemState,
        original_item: LoadOutItem,
        source: &mut LoadOutDataSource,
    ) {
        let _ = source;

        self.sender
            .send(LoadOutTableUiCommand::CellEditComplete(
                cell_index,
                edit_state,
                original_item,
            ))
            .expect("sent");
    }

    fn render_cell_editor(
        &self,
        ui: &mut Ui,
        cell_index: &CellIndex,
        state: &mut Self::ItemState,
        _original_item: &Self::Value,
        _source: &mut LoadOutDataSource,
    ) {
        match state {
            LoadoutItemCellEditState::FeederReference(value) => {
                if !value.is_empty() {
                    Reference::try_from(value.clone())
                        .inspect_err(|_error| {
                            // FUTURE the error could be shown in a tool-tip
                            let validation_error_stroke = Stroke::new(1.0, Color32::RED);
                            ui.painter().rect_stroke(
                                ui.max_rect().shrink(1.0),
                                CornerRadius::ZERO,
                                validation_error_stroke,
                                StrokeKind::Inside,
                            );
                        })
                        .ok();
                }

                let mut value = value.clone();
                if ui
                    .text_edit_singleline(&mut value)
                    .changed()
                {
                    // NOTE: if we had &mut self here, we could apply the edit state now
                    self.sender
                        .send(LoadOutTableUiCommand::ApplyCellEdit {
                            edit: LoadoutItemCellEditState::FeederReference(value),
                            cell_index: *cell_index,
                        })
                        .expect("sent");
                }
            }
        }
    }
}

impl DeferredTableDataSource for LoadOutDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: COLUMN_COUNT,
        }
    }
}

impl DeferredTableRenderer<LoadOutDataSource> for LoadOutRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &LoadOutDataSource) {
        let row = &source.rows[cell_index.row];

        match cell_index.column {
            FEEDER_REFERENCE_COL => ui.label(
                row.reference
                    .as_ref()
                    .map_or("".to_string(), |value| value.to_string()),
            ),
            MANUFACTURER_COL => ui.label(&row.manufacturer),
            MPN_COL => ui.label(&row.mpn),
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
pub struct LoadOutTableUi {
    table_state: Value<(
        LoadOutDataSource,
        LoadOutRenderer,
        LoadOutEditor,
        EditorState<LoadoutItemCellEditState, LoadOutItem>,
    )>,
    #[derivative(Debug = "ignore")]
    filter: Filter,

    pub component: ComponentState<LoadOutTableUiCommand>,
}

impl LoadOutTableUi {
    pub fn new() -> Self {
        let component = ComponentState::default();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(component.sender.clone(), |filter_ui_command| {
                LoadOutTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            table_state: Value::new((
                LoadOutDataSource::new(),
                LoadOutRenderer::new(),
                LoadOutEditor::new(component.sender.clone()),
                EditorState::default(),
            )),
            filter,
            component,
        }
    }

    pub fn update_loadout(&mut self, load_out: LoadOut) {
        let (source, renderer, _editor, _editor_state) = &mut *self.table_state.lock().unwrap();

        let previous_row_count = source.rows.len();

        source.update_loadout(load_out);

        // an ordering is only meaningful for the rows it was made for, if there are now fewer then the table would show
        // some rows twice and not show others.
        if source.rows.len() != previous_row_count {
            renderer.row_ordering = None;
        }
    }

    pub fn filter_ui(&self, ui: &mut Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiCommand {
    None,
    FilterCommand(FilterUiCommand),
    ApplyCellEdit {
        edit: LoadoutItemCellEditState,
        cell_index: CellIndex,
    },
    CellEditComplete(CellIndex, LoadoutItemCellEditState, LoadOutItem),
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiAction {
    None,
    RequestRepaint,
    ItemUpdated {
        index: CellIndex,
        item: LoadOutItem,
        original_item: LoadOutItem,
    },
}

#[derive(Debug, Clone, Default)]
pub struct LoadOutTableUiContext {}

impl UiComponent for LoadOutTableUi {
    type UiContext<'context> = LoadOutTableUiContext;
    type UiCommand = LoadOutTableUiCommand;
    type UiAction = LoadOutTableUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let (source, renderer, editor, editor_state) = &mut *self.table_state.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("load_out_table"))
            .min_size((400.0, 400.0).into())
            .column_parameters(&vec![
                AxisParameters::default()
                    .default_dimension(200.0)
                    .name(tr!("table-load-out-column-reference")),
                AxisParameters::default()
                    .default_dimension(200.0)
                    .name(tr!("table-load-out-column-manufacturer")),
                AxisParameters::default()
                    .default_dimension(200.0)
                    .expandable(true)
                    .name(tr!("table-load-out-column-mpn")),
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
                    apply_reordering(&mut renderer.row_ordering, from, to);
                }
                Action::RowSelectionChanged {
                    ..
                } => {}
            }
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("LoadOutTableUi command. command: {:?}", command);
        match command {
            LoadOutTableUiCommand::None => Some(LoadOutTableUiAction::None),
            LoadOutTableUiCommand::FilterCommand(command) => {
                let action = self
                    .filter
                    .update(command, &mut FilterUiContext::default())
                    .inspect(|action| debug!("filter action: {:?}", action));

                match action {
                    Some(FilterUiAction::ApplyFilter) => {
                        let (source, renderer, _editor, _editor_state) = &mut *self.table_state.lock().unwrap();

                        renderer.rows_to_filter = source
                            .rows
                            .iter()
                            .enumerate()
                            .filter_map(|(id, row)| {
                                let haystack = format!(
                                    "feeder: '{}', manufacturer: '{}', mpn: '{}'",
                                    row.reference
                                        .as_ref()
                                        .map_or("".to_string(), |it| it.to_string()),
                                    row.manufacturer,
                                    row.mpn,
                                );

                                // "Filter single row. If this returns false, the row will be hidden."
                                let result = self.filter.matches(haystack.as_str());

                                trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);

                                if !result { Some(id) } else { None }
                            })
                            .collect::<Vec<usize>>();

                        Some(LoadOutTableUiAction::RequestRepaint)
                    }
                    None => None,
                }
            }

            LoadOutTableUiCommand::ApplyCellEdit {
                edit: new_edit_state,
                cell_index,
            } => {
                let (_source, _renderer, _editor, editor_state) = &mut *self.table_state.lock().unwrap();
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
            LoadOutTableUiCommand::CellEditComplete(cell_index, edit_state, original_item) => {
                let (source, _renderer, _editor, _editor_state) = &mut *self.table_state.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                row.apply_change(edit_state)
                    .map(|_| LoadOutTableUiAction::ItemUpdated {
                        index: cell_index,
                        item: row.clone(),
                        original_item,
                    })
                    .ok()
            }
        }
    }
}
