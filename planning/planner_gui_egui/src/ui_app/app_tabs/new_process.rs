use std::path::PathBuf;
use std::sync::Arc;

use derivative::Derivative;
use egui::{Button, TextEdit, Ui, WidgetText};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{ProcessPresetFactory, ProcessReference};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::file_picker::{FileDialogs, Picker};
use crate::forms::{Form, PickerFieldAction, picker_field};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct NewProcessTab {
    fields: Value<NewProcessFields>,

    #[serde(skip)]
    pub component: ComponentState<NewProcessTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,
}

#[derive(Debug, Clone)]
pub enum NewProcessTabUiCommand {
    ReferenceChanged(String),
    PresetChanged(Option<String>),
    Submit,
    ClearDirectoryClicked,
    PickDirectoryClicked,
    DirectoryPicked(PathBuf),
}

#[derive(Debug)]
pub enum NewProcessTabAction {
    Submit(NewProcessArgs),
}

pub struct NewProcessTabContext {
    pub tab_key: TabKey,
}

impl Tab for NewProcessTab {
    type Context = NewProcessTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("tab-label-new-process"))
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, _context: &mut Self::Context) {
        let mut new_process_tab_context = NewProcessTabContext {
            tab_key: tab_key.clone(),
        };
        UiComponent::ui(self, ui, &mut new_process_tab_context);
    }
}

impl NewProcessTab {
    pub fn new(file_dialogs: Arc<dyn FileDialogs>) -> Self {
        Self {
            file_picker: Value::new(Picker::new(file_dialogs)),
            ..Self::default()
        }
    }
}

impl UiComponent for NewProcessTab {
    type UiContext<'context> = NewProcessTabContext;
    type UiCommand = NewProcessTabUiCommand;
    type UiAction = NewProcessTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_directory) = self
            .file_picker
            .lock()
            .unwrap()
            .picked()
        {
            self.component
                .send(NewProcessTabUiCommand::DirectoryPicked(picked_directory.clone()));
        }

        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let form = Form::new(&self.fields, &self.component.sender, ());

        self.show_form(ui, &form);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            NewProcessTabUiCommand::ReferenceChanged(reference) => {
                self.fields.lock().unwrap().reference = reference;
                None
            }
            NewProcessTabUiCommand::PresetChanged(preset) => {
                self.fields.lock().unwrap().preset = preset;
                None
            }
            NewProcessTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = NewProcessArgs {
                    reference: ProcessReference::from_raw(fields.reference.clone()),
                    directory: fields.directory.clone().unwrap(),
                    preset: fields.preset.clone(),
                };
                Some(NewProcessTabAction::Submit(args))
            }
            NewProcessTabUiCommand::ClearDirectoryClicked => {
                self.fields.lock().unwrap().directory = None;
                None
            }
            NewProcessTabUiCommand::PickDirectoryClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_folder();
                None
            }
            NewProcessTabUiCommand::DirectoryPicked(picked_directory) => {
                self.fields.lock().unwrap().directory = Some(picked_directory);
                None
            }
        }
    }
}

impl NewProcessTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<NewProcessFields, NewProcessTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_process_form"))
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
                    form.add_field_ui("reference", tr!("form-common-input-process-reference"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut reference_clone = fields.reference.clone();
                            let output = TextEdit::singleline(&mut reference_clone)
                                .desired_width(ui.available_width())
                                .hint_text(tr!("form-new-process-input-reference-placeholder"))
                                .show(ui);

                            if !fields.reference.eq(&reference_clone) {
                                sender
                                    .send(NewProcessTabUiCommand::ReferenceChanged(reference_clone))
                                    .expect("sent")
                            }

                            output.response.response
                        }
                    });

                    let directory_label = tr!("form-new-process-input-directory");
                    form.add_field_tui("directory", directory_label.clone(), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            let value = fields
                                .directory
                                .as_ref()
                                .map_or("".to_string(), |path| path.display().to_string());

                            match picker_field(tui, &directory_label, &value) {
                                Some(PickerFieldAction::Clear) => sender
                                    .send(NewProcessTabUiCommand::ClearDirectoryClicked)
                                    .expect("sent"),
                                Some(PickerFieldAction::Pick) => sender
                                    .send(NewProcessTabUiCommand::PickDirectoryClicked)
                                    .expect("sent"),
                                None => {}
                            }
                        }
                    });

                    form.add_field_ui("preset", tr!("form-new-process-input-preset"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let none = tr!("form-common-combo-none");
                            egui::ComboBox::from_id_salt(ui.id().with("preset"))
                                .width(ui.available_width())
                                .selected_text(
                                    fields
                                        .preset
                                        .clone()
                                        .unwrap_or(none.clone()),
                                )
                                .show_ui(ui, |ui| {
                                    if ui
                                        .add(egui::Button::selectable(fields.preset.is_none(), none))
                                        .clicked()
                                    {
                                        sender
                                            .send(NewProcessTabUiCommand::PresetChanged(None))
                                            .expect("sent");
                                    }
                                    for preset in ProcessPresetFactory::available_presets() {
                                        let selected = fields.preset.as_ref() == Some(&preset);
                                        if ui
                                            .add(egui::Button::selectable(selected, preset.clone()))
                                            .clicked()
                                        {
                                            sender
                                                .send(NewProcessTabUiCommand::PresetChanged(Some(preset)))
                                                .expect("sent");
                                        }
                                    }
                                })
                                .response
                        }
                    });
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
                        .send(NewProcessTabUiCommand::Submit);
                }
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
struct NewProcessFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "crate::forms::validation::CommonValidation::validate_reference"))]
    reference: String,

    #[validate(required(code = "form-option-error-required"))]
    directory: Option<PathBuf>,

    /// The preset to start from, otherwise the process has no operations.
    preset: Option<String>,
}

/// Value object
#[derive(Debug, Clone)]
pub struct NewProcessArgs {
    pub reference: ProcessReference,
    pub directory: PathBuf,
    pub preset: Option<String>,
}

impl NewProcessArgs {
    pub fn build_path(&self) -> PathBuf {
        let mut process_file_path: PathBuf = self.directory.clone();
        process_file_path.push(format!("{}.process.json", self.reference));
        process_file_path
    }
}
