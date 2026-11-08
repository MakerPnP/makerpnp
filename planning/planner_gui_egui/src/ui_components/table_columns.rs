//! Each table column's width, chosen by the field it shows, so a table's columns are as wide as their values usually
//! are, the same in every table: a data source's (`VBGUI-11`), a parts tab's (`PARTSRC-3`) and a project's (`FORM-15`).
//!
//! [`column_settings_for_field`] has the default for a field, by its name. A table chooses its columns' settings with
//! [`build_table_settings`], which a table can give its own settings to, for a field with no default of its own, or one
//! it shows differently, and makes them suit `egui_deferred_table`, then has them as the table's column parameters with
//! [`deferred_table_columns`].

use egui::{TextStyle, Ui};
use egui_deferred_table::AxisParameters;

/// A column's width, and whether it takes the table's space left over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSettings {
    pub width: ColumnWidth,
    /// Whether the column is widened to take the width the table's other columns leave, such as a comment's.
    pub use_remaining_space: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// Wide enough for the text, as the table's text is drawn, and for the column's header.
    Fits(&'static str),
    /// In points.
    Points(f32),
}

impl ColumnSettings {
    /// Wide enough for the text, and its header.
    pub const fn fits(text: &'static str) -> Self {
        Self {
            width: ColumnWidth::Fits(text),
            use_remaining_space: false,
        }
    }

    pub const fn points(width: f32) -> Self {
        Self {
            width: ColumnWidth::Points(width),
            use_remaining_space: false,
        }
    }

    /// Also takes the width the other columns leave.
    pub const fn use_remaining_space(mut self) -> Self {
        self.use_remaining_space = true;
        self
    }
}

/// A coordinate's, or a rotation's.
const NUMBER: &str = "123.456";
/// The longest side, `top` or `bottom`.
const SIDE: &str = "bottom";
/// The longest of `true`, `false`, `yes`, `no`, `place` and `no-place`.
const PLACE: &str = "no-place";
const REF_DES: &str = "ABC123";
/// A part's quantity, such as how many a BOM's line needs.
const QUANTITY: &str = "1234";
/// A field an EDA tool's part mapping uses, such as a part's name and value, often long.
const EDA_FIELD: &str = "12345-12345-12345-12345";
/// Any other field.
const OTHER: &str = "ABCDE-ABCDE";

/// The default settings of a field's column, by the field's name, in any case, with or without spaces, `-` or `_`: a
/// coordinate's, a side's, whether it's placed, a ref-des, a quantity, a comment's, which takes the space left, an EDA tool's
/// field's, and any other field's.
pub fn column_settings_for_field(field: &str) -> ColumnSettings {
    let name: String = field
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect();
    match name.as_str() {
        "x" | "y" | "rotation" => ColumnSettings::fits(NUMBER),
        "side" | "pcbside" => ColumnSettings::fits(SIDE),
        "place" => ColumnSettings::fits(PLACE),
        "refdes" => ColumnSettings::fits(REF_DES),
        "quantity" | "qty" => ColumnSettings::fits(QUANTITY),
        "comment" | "comments" | "note" | "notes" => ColumnSettings::fits(OTHER).use_remaining_space(),
        // DipTrace's `name` and `value`, KiCad's `package` and `val`, EasyEDA's `device` and `value`, and footprints.
        "name" | "value" | "val" | "device" | "package" | "footprint" => ColumnSettings::fits(EDA_FIELD),
        _ => ColumnSettings::fits(OTHER),
    }
}

/// Each field's column's settings, in order, the table's own for a field it gives them for, by `table_settings`,
/// otherwise the field's default. `egui_deferred_table` widens only the first column it shows that takes the space left
/// over, which, as the columns can be moved, may not be the one meant, so only one column does: the last that would, or
/// the last column, if none would, so the table fills its width.
pub fn build_table_settings<'a>(
    fields: impl IntoIterator<Item = &'a str>,
    table_settings: impl Fn(&str) -> Option<ColumnSettings>,
) -> Vec<ColumnSettings> {
    let mut settings: Vec<ColumnSettings> = fields
        .into_iter()
        .map(|field| table_settings(field).unwrap_or_else(|| column_settings_for_field(field)))
        .collect();
    let remaining = settings
        .iter()
        .rposition(|settings| settings.use_remaining_space)
        .or(settings.len().checked_sub(1));
    for (index, settings) in settings.iter_mut().enumerate() {
        settings.use_remaining_space = Some(index) == remaining;
    }
    settings
}

/// The columns' parameters, for `egui_deferred_table`, each named by its header, resizable, and as wide as its
/// settings say, a width that fits text being measured as the table draws its text, and its header's, with the cell's
/// margins.
pub fn deferred_table_columns(
    ui: &Ui,
    columns: impl IntoIterator<Item = (String, ColumnSettings)>,
) -> Vec<AxisParameters> {
    columns
        .into_iter()
        .map(|(header, settings)| {
            let width = width_in_points(ui, &header, settings.width);
            AxisParameters::default()
                .name(header)
                .default_dimension(width)
                .resizable(true)
                .expandable(settings.use_remaining_space)
        })
        .collect()
}

/// The table's id, made from the id it'd have and its columns, their names and settings, as `egui_deferred_table` keeps
/// each column's width, by its place, as the user left it, even when the application restarts, and starts a column at
/// its default width only the first time it's shown. So the widths are kept while the columns are the same, and start
/// again from the defaults when they change: when the defaults do, as every table had widths before they had defaults,
/// or when a column is added, or put between others, as an EDA tool's columns are, whose places the kept widths are of.
pub fn table_id(id: egui::Id, columns: &[AxisParameters]) -> egui::Id {
    let columns: Vec<(Option<&str>, Option<u32>, bool, bool)> = columns
        .iter()
        .map(|column| {
            (
                column.name.as_deref(),
                column
                    .default_dimension
                    .map(f32::to_bits),
                column.resizable,
                column.expandable,
            )
        })
        .collect();
    id.with(("columns", columns))
}

/// The cell's margins, either side of its text, and room for the resize handle.
const MARGINS: f32 = 16.0;

fn width_in_points(ui: &Ui, header: &str, width: ColumnWidth) -> f32 {
    match width {
        ColumnWidth::Points(points) => points,
        ColumnWidth::Fits(text) => {
            let font = TextStyle::Body.resolve(ui.style());
            let text_width = |text: &str| {
                ui.painter()
                    .layout_no_wrap(text.to_string(), font.clone(), ui.visuals().text_color())
                    .size()
                    .x
            };
            text_width(text).max(text_width(header)) + MARGINS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FORM-15`, each field's default, by its name, in any case, with or without spaces, `-` or `_`.
    #[test]
    fn a_fields_default_is_chosen_by_its_name() {
        for field in ["X", "y", "Rotation"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("123.456"),
                "{field}"
            );
        }
        for field in ["Side", "PCB side", "pcb_side"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("bottom"),
                "{field}"
            );
        }
        assert_eq!(column_settings_for_field("Place"), ColumnSettings::fits("no-place"));
        for field in ["RefDes", "ref-des", "Ref des"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("ABC123"),
                "{field}"
            );
        }
        for field in ["Quantity", "qty"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("1234"),
                "{field}"
            );
        }
        for field in ["Comment", "notes"] {
            assert!(column_settings_for_field(field).use_remaining_space, "{field}");
        }
        for field in ["name", "Value", "val", "device", "package", "Footprint"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("12345-12345-12345-12345"),
                "{field}"
            );
        }
        for field in ["LCSC", "Manufacturer", "Eda"] {
            assert_eq!(
                column_settings_for_field(field),
                ColumnSettings::fits("ABCDE-ABCDE"),
                "{field}"
            );
        }
    }

    /// `FORM-15`, a table's own settings are used for a field it gives them for, and only one column takes the space
    /// left, as `egui_deferred_table` widens only the first: the last that would, or the last column if none would.
    #[test]
    fn only_one_column_takes_the_space_left() {
        let remaining = |settings: &[ColumnSettings]| -> Vec<bool> {
            settings
                .iter()
                .map(|settings| settings.use_remaining_space)
                .collect()
        };
        let none = |_: &str| None;

        assert_eq!(remaining(&build_table_settings(["RefDes", "Comment", "X"], none)), [
            false, true, false
        ]);
        assert_eq!(remaining(&build_table_settings(["Note", "RefDes", "Comment"], none)), [
            false, false, true
        ]);
        assert_eq!(remaining(&build_table_settings(["RefDes", "X"], none)), [false, true]);
        assert!(build_table_settings([], none).is_empty());

        let mpn_takes_the_space =
            |field: &str| (field == "Mpn").then(|| ColumnSettings::points(200.0).use_remaining_space());
        let settings = build_table_settings(["Manufacturer", "Mpn", "X"], mpn_takes_the_space);
        assert_eq!(remaining(&settings), [false, true, false]);
        assert_eq!(settings[1].width, ColumnWidth::Points(200.0));
        assert_eq!(settings[0], ColumnSettings::fits("ABCDE-ABCDE"));
    }

    /// `FORM-15`, a table's id is the same while its columns are, so the widths the user left are kept, and changes when
    /// a column's default width does, or a column is added, or put between others, so the widths start again from the
    /// defaults.
    #[test]
    fn a_tables_id_changes_with_its_columns() {
        let id = egui::Id::new("table");
        let column = |name: &str, width: f32| {
            AxisParameters::default()
                .name(name)
                .default_dimension(width)
        };
        let before = [column("RefDes", 50.0), column("X", 60.0)];
        assert_eq!(table_id(id, &before), table_id(id, &before.clone()));
        assert_ne!(
            table_id(id, &before),
            table_id(id, &[column("RefDes", 50.0), column("X", 150.0)])
        );
        assert_ne!(
            table_id(id, &before),
            table_id(id, &[column("RefDes", 50.0), column("Name", 60.0), column("X", 60.0)])
        );
        assert_ne!(table_id(id, &before), table_id(egui::Id::new("other"), &before));
    }

    /// `FORM-15`, a column is as wide as its text, or its header, if that's wider, with the cell's margins.
    #[test]
    fn a_column_fits_its_text_and_its_header() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let columns = deferred_table_columns(ui, [
                ("X".to_string(), ColumnSettings::fits("123.456")),
                ("Rotation".to_string(), ColumnSettings::fits("123.456")),
                ("Fixed".to_string(), ColumnSettings::points(42.0)),
            ]);
            let widths: Vec<f32> = columns
                .iter()
                .map(|column| column.default_dimension.unwrap())
                .collect();
            // "Rotation" is wider than "123.456".
            assert!(widths[1] > widths[0], "{widths:?}");
            assert!(widths[0] > MARGINS, "{widths:?}");
            assert_eq!(widths[2], 42.0);
        });
        output.textures_delta.clear();
    }
}
