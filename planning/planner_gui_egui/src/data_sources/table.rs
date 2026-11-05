//! A table of a data source's records, read-only, as the stores core shows any data source other than parts
//! (`STORE-17`).

use std::sync::Arc;

use egui::Ui;
use egui_deferred_table::{
    AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer, TableDimensions,
};
use stores_app::TableView;

/// The view the stores core sent, shared with every other table showing the same data source, not a copy of it.
struct TableDataSource {
    view: Arc<TableView>,
}

impl DeferredTableDataSource for TableDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.view.table.rows.len(),
            column_count: self.view.table.columns.len(),
        }
    }
}

struct TableRenderer {}

impl DeferredTableRenderer<TableDataSource> for TableRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &TableDataSource) {
        let value = source.view.table.rows[cell_index.row]
            .get(cell_index.column)
            .map_or("", |value| value.as_str());
        ui.label(value);
    }
}

/// Shows the table, a row for each of the data source's rows, with its columns as the headers, taking all the space
/// left in the `Ui`, so it's resized with its container. Read-only, so it has no commands of its own yet.
#[profiling::function]
pub fn show_table(ui: &mut Ui, id: egui::Id, view: Arc<TableView>) {
    let mut column_parameters: Vec<AxisParameters> = view
        .table
        .columns
        .iter()
        .map(|name| {
            AxisParameters::default()
                .default_dimension(150.0)
                .name(name.clone())
        })
        .collect();
    if let Some(last) = column_parameters.pop() {
        column_parameters.push(last.expandable(true));
    }

    let mut source = TableDataSource {
        view,
    };
    let mut renderer = TableRenderer {};

    // FUTURE selecting a row selects the corresponding rows of other tables, so the actions will be handled.
    let (_response, _actions) = DeferredTable::new(id)
        .min_size((100.0, 100.0).into())
        .column_parameters(&column_parameters)
        .show(ui, &mut source, &mut renderer);
}
