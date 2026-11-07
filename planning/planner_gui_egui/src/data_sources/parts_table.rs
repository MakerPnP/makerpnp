//! A table of the parts of a parts data source, with each part's other values.

use std::sync::Arc;

use egui::Ui;
use egui_deferred_table::{
    AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer, TableDimensions,
};
use egui_i18n::tr;
use stores_app::PartsView;

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

/// Shows the parts. Read-only, so it has no commands of its own yet.
#[profiling::function]
pub fn show_parts_table(ui: &mut Ui, view: Arc<PartsView>) {
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
    super::in_space_left(ui, |ui| {
        let (_response, _actions) = DeferredTable::new(ui.make_persistent_id("parts_source_table"))
            .min_size((400.0, 400.0).into())
            .column_parameters(&column_parameters)
            .show(ui, &mut source, &mut renderer);
    });
}
