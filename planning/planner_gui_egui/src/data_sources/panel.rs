//! The GUI's one data source panel, used wherever a data source is shown, a variant builder's panels and a parts tab, so
//! a data source is shown, and changed, the same way in each (`VBGUI-11`, `PARTSRC-3`): its file, "Reload", why it
//! can't be read, or why its last change was refused, the row buttons of a table that can be changed, and the table.

use std::collections::{BTreeMap, BTreeSet};

use eda_units::eda_units::angle::AngleUnit;
use eda_units::eda_units::dimension_unit::{DimensionUnitPoint2, DimensionUnitPoint2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use egui::Ui;
use egui_i18n::tr;
use planner_app::{PcbSide, PlacementPositionUnit};
use stores_app::{ColumnsToAdd, Event, Source, Table, TableKind, TableRow, TableView};

use crate::data_sources::table::{
    TableEditing, TableRequest, TableState, TableUiCommand, is_changed_here, show_row_buttons, show_table, update_table,
};
use crate::stores_core::StoresCoreService;
use crate::sync::Value;
use crate::ui_components::errors::{Errors, ErrorsUiCommand, show_information};

/// The panel's state, kept by the tab showing it, for each table it shows, across frames, and the tables it has had
/// the stores core open.
#[derive(Debug, Default)]
pub struct DataSourcePanel {
    /// Each changeable table's own state, as egui keeps a widget's.
    editors: Value<BTreeMap<(TableKind, Source), TableState>>,
    /// Asked for once, as a table is loaded once, and shown by every view of it (`STORE-5`).
    opened: BTreeSet<(TableKind, Source)>,
}

/// The panel's commands, for the tab to log and handle with [`DataSourcePanel::update`].
#[derive(Debug, Clone)]
pub enum DataSourcePanelUiCommand {
    /// Has the stores core open the table, if it hasn't been asked to (`STORE-17`).
    Open(TableKind, Source),
    /// "Reload", has the stores core read the table again (`STORE-17`).
    Reload(TableKind, Source),
    /// The table's command, such as a cell edited (`VBGUI-13`).
    Table(TableKind, Source, TableUiCommand),
    /// A placement's row was selected, an EDA placement's or a variant's, to locate it (`VBGUI-18`).
    PlacementSelected(PlacementToLocate),
}

/// What the panel asks of the tab showing it.
#[derive(Debug, Clone)]
pub enum DataSourcePanelUiAction {
    /// Locate the placement on the PCBs its design is on (`VBGUI-18`).
    LocatePlacement(PlacementToLocate),
}

/// A placement to locate: its ref-des, its side, and its position, as the EDA tool exported it (`VBGUI-18`).
#[derive(Debug, Clone)]
pub struct PlacementToLocate {
    pub ref_des: String,
    pub pcb_side: PcbSide,
    pub design_position: PlacementPositionUnit,
}

impl PlacementToLocate {
    /// The placement shown in the row, if the table is of placements, an EDA tool's or a variant's.
    fn of_row(view: &TableView, row: usize) -> Option<Self> {
        let (ref_des, pcb_side, x, y, rotation) = match &view.table {
            Table::EdaPlacements(items) => {
                let placement = &items.get(row)?.placement;
                (
                    placement.ref_des.clone(),
                    placement.pcb_side.clone(),
                    placement.x,
                    placement.y,
                    placement.rotation,
                )
            }
            Table::Placements(items) => {
                let placement = &items.get(row)?.placement;
                (
                    placement.ref_des.to_string(),
                    placement.pcb_side.clone(),
                    placement.x,
                    placement.y,
                    placement.rotation,
                )
            }
            _ => return None,
        };
        Some(Self {
            ref_des,
            pcb_side,
            // FIXME hard-coded use of UnitSystem::Millimeters, as a project's placements table has.
            design_position: PlacementPositionUnit::new(
                DimensionUnitPoint2::new_dim_decimal(x, y, UnitSystem::Millimeters),
                AngleUnit::new_degrees_decimal(rotation),
            ),
        })
    }
}

impl DataSourcePanelUiCommand {
    /// Whether the user did it, such as clicking "Reload", or editing a cell, so the tab shows its dismissed errors
    /// again, as it may have caused them again, rather than the panel itself, such as opening the table, or selecting
    /// the rows a change made once it's shown (`FORM-13`).
    pub fn is_users(&self) -> bool {
        match self {
            DataSourcePanelUiCommand::Open(..) => false,
            DataSourcePanelUiCommand::Reload(..) => true,
            DataSourcePanelUiCommand::Table(_, _, command) => command.is_users(),
            DataSourcePanelUiCommand::PlacementSelected(_) => true,
        }
    }
}

impl DataSourcePanel {
    /// Handles the panel's command, having the stores core do what it asks, if anything, or the tab, by its action.
    pub fn update(
        &mut self,
        command: DataSourcePanelUiCommand,
        stores: &Value<StoresCoreService>,
    ) -> Option<DataSourcePanelUiAction> {
        let event = match command {
            DataSourcePanelUiCommand::PlacementSelected(placement) => {
                return Some(DataSourcePanelUiAction::LocatePlacement(placement));
            }
            DataSourcePanelUiCommand::Open(kind, source) => {
                if !self
                    .opened
                    .insert((kind, source.clone()))
                {
                    return None;
                }
                Event::OpenTable {
                    kind,
                    source,
                }
            }
            // every view of the data source shows what's read (`PARTSRC-5`).
            DataSourcePanelUiCommand::Reload(kind, source) => Event::LoadTable {
                kind,
                source,
            },
            DataSourcePanelUiCommand::Table(kind, source, command) => {
                let request = {
                    let mut editors = self.editors.lock().unwrap();
                    let state = editors
                        .entry((kind, source.clone()))
                        .or_default();
                    update_table(state, command)
                };
                // `STORE-19`, written at once, so every view of the data source shows it, or, `STORE-21`, the columns
                // a row's change needs added first.
                match request {
                    Some(TableRequest::Change(change)) => Event::ChangeTable {
                        kind,
                        source,
                        change,
                    },
                    Some(TableRequest::AddColumns(eda_tool)) => Event::AddTableColumns {
                        kind,
                        source,
                        columns: ColumnsToAdd::EdaTool {
                            eda_tool,
                        },
                    },
                    None => return None,
                }
            }
        };
        stores.lock().unwrap().process(event);
        None
    }

    /// Shows the table, filling the space left, once it's loaded, and why it can't be, having the stores core open it if
    /// it hasn't (`STORE-17`). One whose kind can be changed is edited, with the row buttons above it, "Add row" adding
    /// `new_row` (`VBGUI-13`).
    pub fn show(
        &self,
        ui: &mut Ui,
        stores: &Value<StoresCoreService>,
        kind: TableKind,
        source: &Source,
        new_row: Option<TableRow>,
        errors: &Errors,
        send: &dyn Fn(DataSourcePanelUiCommand),
        send_errors: &dyn Fn(ErrorsUiCommand),
    ) {
        // the latest view, whichever tab or panel loaded it, shared, not copied (`PARTSRC-5`).
        let (status, view) = {
            let stores = stores.lock().unwrap();
            let status = stores.table_status(kind, source);
            let view = status
                .as_ref()
                .and_then(|status| stores.table_view(kind, &status.source));
            (status, view)
        };
        if status.is_none() {
            send(DataSourcePanelUiCommand::Open(kind, source.clone()));
        }

        // as the stores service identified it, once it has (`STORE-6`).
        let shown_source = status
            .as_ref()
            .map_or(source, |status| &status.source);
        ui.horizontal(|ui| {
            ui.label(tr!("data-source-panel-path", { path: shown_source.to_string() }));
            if ui
                .button(tr!("data-source-panel-button-reload"))
                .clicked()
            {
                send(DataSourcePanelUiCommand::Reload(kind, source.clone()));
            }
        });
        // `VBGUI-17`, the EDA tool writes its placements, so they're changed there, not here, first, as it always
        // applies.
        if matches!(kind, TableKind::EdaPlacements { .. }) {
            show_information(ui, &tr!("data-source-panel-eda-placements-autogenerated"));
        }
        // the data source's, so shown by every view of it, each with the table loaded before, if any (`VBGUI-11`), each
        // error in the tab's error frame, which the user can dismiss (`FORM-12`, `FORM-13`).
        if let Some(error) = status
            .as_ref()
            .and_then(|status| status.error.as_ref())
        {
            errors.show(ui, &format!("{kind:?} {source}"), error, send_errors);
        }
        // `VBGUI-13`, why the last change was refused, until one succeeds.
        if let Some(error) = status
            .as_ref()
            .and_then(|status| status.change_error.as_ref())
        {
            errors.show(ui, &format!("{kind:?} {source} change"), error, send_errors);
        }

        match view {
            Some(view) if is_changed_here(&view) => {
                let changing = status
                    .as_ref()
                    .is_some_and(|status| status.changing);
                // a change is written, then the table's read again, so its view shows it once neither is under way.
                let reading = status
                    .as_ref()
                    .is_some_and(|status| status.reading);
                let send_table =
                    |command: TableUiCommand| send(DataSourcePanelUiCommand::Table(kind, source.clone(), command));
                let mut editors = self.editors.lock().unwrap();
                let state = editors
                    .entry((kind, source.clone()))
                    .or_default();
                show_row_buttons(ui, &view, state, new_row, changing, &send_table);
                show_table(
                    ui,
                    ui.id().with("table"),
                    view,
                    Some(TableEditing {
                        state,
                        send: &send_table,
                        busy: changing || reading,
                        refused: status
                            .as_ref()
                            .is_some_and(|status| status.change_error.is_some()),
                    }),
                );
            }
            Some(view) => {
                // `VBGUI-18`, a placement selected is located, the EDA tool's, which aren't changed here, or the
                // variant's.
                if let Some(placement) = show_table(ui, ui.id().with("table"), view.clone(), None)
                    .and_then(|row| PlacementToLocate::of_row(&view, row))
                {
                    send(DataSourcePanelUiCommand::PlacementSelected(placement));
                }
            }
            // the store operations are done without the GUI waiting for them, so the table may not be read yet.
            None if status
                .as_ref()
                .is_none_or(|status| status.reading) =>
            {
                ui.spinner();
            }
            None => {}
        }
    }
}
