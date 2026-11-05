//! A table of a data source's records, read-only, as the stores core shows any data source other than parts
//! (`STORE-17`), each value shown as what it is, a pattern by its kind (`VBGUI-11`).

use std::sync::Arc;

use egui::{Color32, Ui, Visuals};
use egui_deferred_table::{
    AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer, TableDimensions,
};
use stores_app::{Cell, Pattern, TableView};

/// The view the stores core sent, shared with every other table showing the same data source, not a copy of it.
struct TableDataSource {
    view: Arc<TableView>,
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
/// all the space left in the `Ui`, so it's resized with its container. Read-only, so it has no commands of its own yet.
#[profiling::function]
pub fn show_table(ui: &mut Ui, id: egui::Id, view: Arc<TableView>) {
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

#[cfg(test)]
mod tests {
    use egui::Visuals;
    use stores_app::Pattern;

    use super::pattern_color;

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
