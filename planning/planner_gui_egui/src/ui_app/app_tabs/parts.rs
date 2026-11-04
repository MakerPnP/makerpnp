//! A parts data source's tab: first the form to choose the data source, then its parts (`PARTSRC`).

use std::path::PathBuf;
use std::sync::Arc;

use derivative::Derivative;
use egui::{Button, TextEdit, Ui, Widget, WidgetText};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use serde::{Deserialize, Serialize};
use stores_app::{Event, Part, PartsChange, PartsSource, PartsSourceStatus, PartsView};
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::data_sources::parts_table::show_parts_table;
use crate::file_picker::{FileDialogs, Picker};
use crate::forms::{Form, NamedResponse, PickerFieldAction, picker_field};
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

    /// The part to add or remove, as typed (`PARTSRC-6`).
    #[serde(skip)]
    manufacturer: String,
    #[serde(skip)]
    mpn: String,
}

#[derive(Debug, Clone)]
pub enum PartsTabUiCommand {
    SourceKindChanged(PartsSourceKind),
    ClearCsvFileClicked,
    PickCsvFileClicked,
    CsvFilePicked(PathBuf),
    Submit,
    ReloadClicked,
    ManufacturerChanged(String),
    MpnChanged(String),
    AddPartClicked,
    RemovePartClicked,

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

    /// Processes the event for the tab's data source. The parts the core sends, and whether they could be loaded, are
    /// kept by the stores core service, for every tab showing them, so there is nothing more to do.
    fn process(&mut self, stores: &Value<StoresCoreService>, event: impl FnOnce(PartsSource) -> Event) {
        let Some(source) = &self.source else {
            return;
        };

        stores
            .lock()
            .unwrap()
            .process(event(source.clone()));
    }

    /// The part typed, if both its manufacturer and MPN are.
    fn part(&self) -> Option<Part> {
        let (manufacturer, mpn) = (self.manufacturer.trim(), self.mpn.trim());
        (!manufacturer.is_empty() && !mpn.is_empty()).then(|| Part::new(manufacturer.to_string(), mpn.to_string()))
    }

    /// Has the stores core make the change to the parts shown, based on the revision of the view shown, so that it's
    /// refused if the data source has changed since (`STORE-15`).
    fn change(&self, stores: &Value<StoresCoreService>, change: impl FnOnce(Part) -> PartsChange) {
        let (Some(source), Some(part)) = (&self.source, self.part()) else {
            return;
        };
        let mut stores = stores.lock().unwrap();
        let Some(revision) = stores
            .parts_source_status(source)
            .and_then(|status| stores.parts_view(&status.source))
            .map(|view| view.revision.clone())
        else {
            // the buttons are disabled until the parts are shown.
            return;
        };

        stores.process(Event::ChangeParts {
            source: source.clone(),
            revision,
            change: change(part),
        });
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
            Some(source) => {
                // the latest parts, whichever tab loaded them, shared, not copied (`PARTSRC-5`).
                let (status, view) = {
                    let stores = context.stores.lock().unwrap();
                    let status = stores.parts_source_status(source);
                    let view = status
                        .as_ref()
                        .and_then(|status| stores.parts_view(&status.source));
                    (status, view)
                };
                self.show_parts(ui, source, status, view);
            }
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
                self.process(&context.stores, |source| Event::OpenParts {
                    source,
                });
                None
            }
            PartsTabUiCommand::ReloadClicked => {
                // every tab showing the data source shows the parts loaded (`PARTSRC-5`).
                self.process(&context.stores, |source| Event::LoadParts {
                    source,
                });
                None
            }
            PartsTabUiCommand::ManufacturerChanged(manufacturer) => {
                self.manufacturer = manufacturer;
                None
            }
            PartsTabUiCommand::MpnChanged(mpn) => {
                self.mpn = mpn;
                None
            }
            PartsTabUiCommand::AddPartClicked => {
                self.change(&context.stores, |part| PartsChange::Add {
                    part,
                    fields: Default::default(),
                });
                None
            }
            PartsTabUiCommand::RemovePartClicked => {
                self.change(&context.stores, |part| PartsChange::Remove {
                    part,
                });
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

    /// `status` is what the core knows of the data source, if it's been asked for it, and `view` its parts, if they're
    /// loaded.
    fn show_parts(
        &self,
        ui: &mut Ui,
        source: &PartsSource,
        status: Option<PartsSourceStatus>,
        view: Option<Arc<PartsView>>,
    ) {
        // as the stores service identified it, once it has (`STORE-6`).
        let shown_source = status
            .as_ref()
            .map_or(source, |status| &status.source);

        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr!("parts-source-csv-file", { path: shown_source.to_string() }));
                    if ui
                        .button(tr!("parts-source-button-reload"))
                        .clicked()
                    {
                        self.component
                            .send(PartsTabUiCommand::ReloadClicked);
                    }
                });

                // the data source's, so shown by every tab showing it, each with the parts loaded before, if any.
                if let Some(error) = status
                    .as_ref()
                    .and_then(|status| status.error.as_ref())
                {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                if let (Some(status), Some(view)) = (status.as_ref(), view.as_ref()) {
                    self.show_change(ui, status, view);
                }

                ui.separator();

                // the store operations are done without the GUI waiting for them, so the parts may not be read yet.
                let reading = status
                    .as_ref()
                    .is_none_or(|status| status.reading);
                let Some(view) = view else {
                    if reading {
                        ui.label(tr!("parts-source-loading"));
                    }
                    return;
                };

                // `FORM-8`, an empty table isn't drawn.
                if view.parts.is_empty() {
                    ui.label(tr!("parts-source-no-parts"));
                } else {
                    show_parts_table(ui, view);
                }
            });
    }
}

impl PartsTab {
    /// The part to add or remove, and the buttons that do it, enabled only for a part that is, or isn't, shown, and
    /// while no other change is being made, and why the data source's last change was refused, if it was
    /// (`PARTSRC-6`, `STORE-15`).
    fn show_change(&self, ui: &mut Ui, status: &PartsSourceStatus, view: &PartsView) {
        ui.horizontal(|ui| {
            let manufacturer_label = tr!("parts-source-input-manufacturer");
            let mut manufacturer = self.manufacturer.clone();
            if TextEdit::singleline(&mut manufacturer)
                .hint_text(manufacturer_label.clone())
                .desired_width(150.0)
                .ui(ui)
                .named(manufacturer_label)
                .changed()
            {
                self.component
                    .send(PartsTabUiCommand::ManufacturerChanged(manufacturer));
            }

            let mpn_label = tr!("parts-source-input-mpn");
            let mut mpn = self.mpn.clone();
            if TextEdit::singleline(&mut mpn)
                .hint_text(mpn_label.clone())
                .desired_width(150.0)
                .ui(ui)
                .named(mpn_label)
                .changed()
            {
                self.component
                    .send(PartsTabUiCommand::MpnChanged(mpn));
            }

            let shown = self.part().map(|part| {
                view.parts
                    .iter()
                    .any(|item| item.part == part)
            });
            let can_change = !status.changing;

            if ui
                .add_enabled(
                    can_change && shown == Some(false),
                    Button::new(tr!("parts-source-button-add-part")),
                )
                .clicked()
            {
                self.component
                    .send(PartsTabUiCommand::AddPartClicked);
            }
            if ui
                .add_enabled(
                    can_change && shown == Some(true),
                    Button::new(tr!("parts-source-button-remove-part")),
                )
                .clicked()
            {
                self.component
                    .send(PartsTabUiCommand::RemovePartClicked);
            }
        });

        // the data source's, so shown by every tab showing it, until a change succeeds.
        if let Some(error) = &status.change_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
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
