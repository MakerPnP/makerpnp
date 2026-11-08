//! A parts data source's tab: first the form to choose the data source, then its parts, shown, and changed, with the
//! GUI's one data source panel, as a variant builder's "Parts" panel shows them (`PARTSRC`, `VBGUI-16`).

use std::path::PathBuf;
use std::sync::Arc;

use derivative::Derivative;
use egui::{Button, Ui, WidgetText};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use serde::{Deserialize, Serialize};
use stores_app::{PartsSource, TableKind};
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::data_sources::panel::{DataSourcePanel, DataSourcePanelUiCommand};
use crate::data_sources::table::new_row;
use crate::file_picker::{FileDialogs, Picker};
use crate::forms::{Form, PickerFieldAction, picker_field};
use crate::stores_core::StoresCoreService;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

/// The kinds of data source the user can choose from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartsSourceKind {
    /// Nothing chosen yet.
    #[default]
    None,
    CsvFile,
}

/// The tab is persisted, so that it can be shown again when the application restarts.
#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct PartsTab {
    fields: Value<PartsSourceFields>,

    /// The data source chosen with "Ok", as it was chosen. Until then the form is shown. The stores service identifies
    /// it, for example by its file's canonical path (`STORE-6`), and the tab shows it as identified.
    pub source: Option<PartsSource>,

    #[serde(skip)]
    pub component: ComponentState<PartsTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,

    /// The data source panel's state: whether it has had the stores core open the parts, and the cell being edited,
    /// and the rows selected (`PARTSRC-6`).
    #[serde(skip)]
    #[derivative(Debug = "ignore")]
    data_source_panel: DataSourcePanel,
}

#[derive(Debug, Clone)]
pub enum PartsTabUiCommand {
    SourceKindChanged(PartsSourceKind),
    ClearCsvFileClicked,
    PickCsvFileClicked,
    CsvFilePicked(PathBuf),
    Submit,
    /// The data source panel's command, such as "Reload", or a part's value edited (`PARTSRC-6`).
    Panel(DataSourcePanelUiCommand),

    // internal
    /// Opens the data source, using its parts if they're loaded already, as when another tab shows them.
    Open,
}

#[derive(Debug)]
pub enum PartsTabAction {
    /// Commands for this tab to handle next, in order.
    Task(Task<PartsTabUiCommand>),
}

pub struct PartsTabContext {
    pub tab_key: TabKey,
    pub stores: Value<StoresCoreService>,
}

impl PartsTab {
    pub fn new(file_dialogs: Arc<dyn FileDialogs>) -> Self {
        Self {
            file_picker: Value::new(Picker::new(file_dialogs)),
            ..Self::default()
        }
    }

    /// For a tab restored when the application starts, which has no file dialogs of its own.
    pub fn set_file_dialogs(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        self.file_picker = Value::new(Picker::new(file_dialogs));
    }
}

impl Tab for PartsTab {
    type Context = PartsTabContext;

    fn label(&self) -> WidgetText {
        let label = self
            .source
            .as_ref()
            .and_then(|source| source.path().ok())
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| tr!("tab-label-open-parts"));

        WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        let mut parts_tab_context = PartsTabContext {
            tab_key: *tab_key,
            stores: context.stores.clone(),
        };
        UiComponent::ui(self, ui, &mut parts_tab_context);
    }
}

impl UiComponent for PartsTab {
    type UiContext<'context> = PartsTabContext;
    type UiCommand = PartsTabUiCommand;
    type UiAction = PartsTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        match &self.source {
            None => {
                if let Ok(picked_file) = self
                    .file_picker
                    .lock()
                    .unwrap()
                    .picked()
                {
                    self.component
                        .send(PartsTabUiCommand::CsvFilePicked(picked_file));
                }

                let form = Form::new(&self.fields, &self.component.sender, ());
                self.show_form(ui, &form);
            }
            Some(source) => self.show_parts(ui, source, &context.stores),
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("PartsTab command. command: {:?}", command);
        match command {
            PartsTabUiCommand::SourceKindChanged(kind) => {
                self.fields.lock().unwrap().kind = kind;
                None
            }
            PartsTabUiCommand::ClearCsvFileClicked => {
                self.fields.lock().unwrap().csv_file = None;
                None
            }
            PartsTabUiCommand::PickCsvFileClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file();
                None
            }
            PartsTabUiCommand::CsvFilePicked(path) => {
                self.fields.lock().unwrap().csv_file = Some(path);
                None
            }
            PartsTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let source = match (fields.kind, &fields.csv_file) {
                    (PartsSourceKind::CsvFile, Some(path)) => PartsSource::File(path.clone()),
                    // "Ok" is disabled until the form is valid.
                    _ => return None,
                };
                drop(fields);

                self.source = Some(source);
                Some(PartsTabAction::Task(Task::done(PartsTabUiCommand::Open)))
            }
            PartsTabUiCommand::Open => {
                if let Some(source) = &self.source {
                    // the parts as a table, loaded once however many tabs and panels show them (`PARTSRC-5`).
                    self.data_source_panel.update(
                        DataSourcePanelUiCommand::Open(TableKind::Parts, source.clone()),
                        &context.stores,
                    );
                }
                None
            }
            PartsTabUiCommand::Panel(command) => {
                self.data_source_panel
                    .update(command, &context.stores);
                None
            }
        }
    }
}

impl PartsTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<PartsSourceFields, PartsTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        let kind = self.fields.lock().unwrap().kind;

        tui(ui, ui.id().with("open_parts_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("kind", tr!("form-open-parts-input-source"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let label = |kind: PartsSourceKind| match kind {
                                PartsSourceKind::None => tr!("form-open-parts-source-none"),
                                PartsSourceKind::CsvFile => tr!("form-open-parts-source-csv-file"),
                            };
                            egui::ComboBox::from_id_salt(ui.id().with("kind"))
                                .width(ui.available_width())
                                .selected_text(label(fields.kind))
                                .show_ui(ui, |ui| {
                                    for kind in [PartsSourceKind::None, PartsSourceKind::CsvFile] {
                                        if ui
                                            .add(egui::Button::selectable(fields.kind == kind, label(kind)))
                                            .clicked()
                                        {
                                            sender
                                                .send(PartsTabUiCommand::SourceKindChanged(kind))
                                                .expect("sent");
                                        }
                                    }
                                })
                                .response
                        }
                    });

                    if kind == PartsSourceKind::CsvFile {
                        form.add_section_tui("csv_file", tr!("form-open-parts-group-csv-file"), tui, |tui| {
                            form.show_fields_vertical(tui, |form, tui| {
                                let file_label = tr!("form-open-parts-input-csv-file");
                                form.add_field_tui("csv_file", file_label.clone(), tui, {
                                    move |tui: &mut Tui, fields, sender| {
                                        let value = fields
                                            .csv_file
                                            .as_ref()
                                            .map_or("".to_string(), |path| path.display().to_string());

                                        match picker_field(tui, &file_label, &value) {
                                            Some(PickerFieldAction::Clear) => sender
                                                .send(PartsTabUiCommand::ClearCsvFileClicked)
                                                .expect("sent"),
                                            Some(PickerFieldAction::Pick) => sender
                                                .send(PartsTabUiCommand::PickCsvFileClicked)
                                                .expect("sent"),
                                            None => {}
                                        }
                                    }
                                });
                            });
                        });
                    }
                });

                if tui
                    .style(Style {
                        ..default_style()
                    })
                    .enabled_ui(form.is_valid())
                    .ui_add(Button::new(tr!("form-common-button-ok")))
                    .clicked()
                {
                    self.component
                        .send(PartsTabUiCommand::Submit);
                }
            });
    }

    /// The parts, with the GUI's one data source panel, which has the stores core open them if it hasn't, and changes
    /// them, "Add row" adding a part with every value empty (`PARTSRC-3`, `PARTSRC-6`, `VBGUI-16`).
    fn show_parts(&self, ui: &mut Ui, source: &PartsSource, stores: &Value<StoresCoreService>) {
        let send = |command: DataSourcePanelUiCommand| {
            self.component
                .send(PartsTabUiCommand::Panel(command))
        };
        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                self.data_source_panel.show(
                    ui,
                    stores,
                    TableKind::Parts,
                    source,
                    new_row(TableKind::Parts, None),
                    &send,
                );
            });
    }
}

#[derive(Clone, Debug, Default, Validate, Serialize, Deserialize)]
struct PartsSourceFields {
    #[validate(custom(function = "PartsSourceFields::validate_kind"))]
    kind: PartsSourceKind,

    /// Only shown, and required, when the kind is a CSV file, the only kind there is with a file.
    #[validate(required(code = "form-option-error-required"))]
    csv_file: Option<PathBuf>,
}

impl PartsSourceFields {
    fn validate_kind(kind: &PartsSourceKind) -> Result<(), ValidationError> {
        match kind {
            PartsSourceKind::None => Err(ValidationError::new("form-option-error-required")),
            PartsSourceKind::CsvFile => Ok(()),
        }
    }
}
