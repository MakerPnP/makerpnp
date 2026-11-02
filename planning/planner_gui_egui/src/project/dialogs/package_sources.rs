use std::fmt::Debug;
use std::path::PathBuf;

use derivative::Derivative;
use egui::{Button, Frame, Modal, TextEdit, Ui, Widget};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic, taffy, tui};
use planner_app::{PackageMappingsSource, PackagesSource};
use taffy::Size;
use tracing::error;
use validator::Validate;

use crate::file_picker::{PickError, Picker};
use crate::forms::Form;
use crate::forms::transforms::resize_x_transform;
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickReason {
    PackageSource,
    PackageMappingsSource,
}

impl PickReason {
    pub fn file_filter(self) -> &'static str {
        match self {
            PickReason::PackageSource => "*.csv",
            PickReason::PackageMappingsSource => "*.csv",
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PackageSourcesModal {
    fields: Value<PackageSourcesFields>,
    path: PathBuf,

    #[derivative(Debug = "ignore")]
    file_picker: Value<
        Option<(
            PickReason,
            Picker,
            Box<dyn Fn(PathBuf) -> PackageSourcesModalUiCommand + Send + Sync + 'static>,
        )>,
    >,

    pub component: ComponentState<PackageSourcesModalUiCommand>,
}

impl PackageSourcesModal {
    pub fn new(
        path: PathBuf,
        packages_source: Option<PackagesSource>,
        package_mappings_source: Option<PackageMappingsSource>,
    ) -> Self {
        let fields = PackageSourcesFields {
            packages_source,
            package_mappings_source,
            ..PackageSourcesFields::default()
        };
        Self {
            fields: Value::new(fields),
            path,
            component: Default::default(),
            file_picker: Default::default(),
        }
    }

    // FUTURE consider returning a result to indicate if the picker was busy
    fn pick_file(
        &mut self,
        reason: PickReason,
        command_fn: Box<dyn Fn(PathBuf) -> PackageSourcesModalUiCommand + Send + Sync + 'static>,
    ) {
        // TODO use the filter, picker API needs updating
        let _filter = reason.file_filter();

        let mut file_picker = self.file_picker.lock().unwrap();

        if file_picker.is_some() {
            error!("file picker busy, not picking a {:?}", reason);
        } else {
            let mut picker = Picker::default();
            picker.pick_file();
            *file_picker = Some((reason, picker, command_fn));
        }
    }

    fn pick_packages_file(&mut self) {
        let open_project_file_command_fn = |path: PathBuf| PackageSourcesModalUiCommand::PackagesSourcePicked(path);

        self.pick_file(PickReason::PackageSource, Box::new(open_project_file_command_fn));
    }

    fn pick_package_mappings_file(&mut self) {
        let open_pcb_file_command_fn = |path: PathBuf| PackageSourcesModalUiCommand::PackageMappingsSourcePicked(path);
        self.pick_file(PickReason::PackageMappingsSource, Box::new(open_pcb_file_command_fn));
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<PackageSourcesFields, PackageSourcesModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("package_sources_form"))
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
                    form.add_field_tui(
                        "packages_source",
                        tr!("form-package-sources-input-packages-source"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add_with_border(|tui| {
                                    tui.style(Style {
                                        flex_grow: 1.0,
                                        ..default_style()
                                    })
                                    .ui_add_manual(
                                        |ui| {
                                            // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                            //      as a workaround we use `ui_add_manual` for now, with `no_transform`.

                                            let mut chosen_path = fields
                                                .packages_source
                                                .as_ref()
                                                .map(|s| s.to_string())
                                                .unwrap_or_default();

                                            // FUTURE consider making this interactive since loadout sources do not have to be files. (in the future they could be urls, etc)
                                            TextEdit::singleline(&mut chosen_path)
                                                .desired_width(ui.available_width())
                                                .interactive(false)
                                                .frame(Frame::NONE)
                                                .ui(ui)
                                        },
                                        resize_x_transform,
                                    );

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .ui_add(Button::new("x"))
                                        .clicked()
                                    {
                                        sender
                                            .send(PackageSourcesModalUiCommand::ClearPackagesSourceClicked)
                                            .expect("sent");
                                    }

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .ui_add(Button::new("..."))
                                        .clicked()
                                    {
                                        sender
                                            .send(PackageSourcesModalUiCommand::PickPackagesSourceClicked)
                                            .expect("sent");
                                    }
                                })
                            }
                        },
                    );

                    form.add_field_tui(
                        "package_mappings_source",
                        tr!("form-package-sources-input-package-mappings-source"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add_with_border(|tui| {
                                    tui.style(Style {
                                        flex_grow: 1.0,
                                        ..default_style()
                                    })
                                    .ui_add_manual(
                                        |ui| {
                                            // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                            //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                                            let mut chosen_path = fields
                                                .package_mappings_source
                                                .as_ref()
                                                .map(|s| s.to_string())
                                                .unwrap_or_default();

                                            // FUTURE consider making this interactive since loadout sources do not have to be files. (in the future they could be urls, etc)
                                            TextEdit::singleline(&mut chosen_path)
                                                .desired_width(ui.available_width())
                                                .interactive(false)
                                                .frame(Frame::NONE)
                                                .ui(ui)
                                        },
                                        resize_x_transform,
                                    );

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .ui_add(Button::new("x"))
                                        .clicked()
                                    {
                                        sender
                                            .send(PackageSourcesModalUiCommand::ClearPackageMappingsSourceClicked)
                                            .expect("sent");
                                    }

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .ui_add(Button::new("..."))
                                        .clicked()
                                    {
                                        sender
                                            .send(PackageSourcesModalUiCommand::PickPackageMappingsSourceClicked)
                                            .expect("sent");
                                    }
                                })
                            }
                        },
                    );
                });
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct PackageSourcesFields {
    // both fields are optional, the user can set both, one now and the other later and also remove the sources
    packages_source: Option<PackagesSource>,
    package_mappings_source: Option<PackageMappingsSource>,
}

#[derive(Debug, Clone)]
pub enum PackageSourcesModalUiCommand {
    Submit,
    Cancel,

    PickPackagesSourceClicked,
    PickPackageMappingsSourceClicked,
    PackagesSourcePicked(PathBuf),
    PackageMappingsSourcePicked(PathBuf),
    ClearPackagesSourceClicked,
    ClearPackageMappingsSourceClicked,
}

#[derive(Debug, Clone)]
pub enum PackageSourcesModalAction {
    Submit(PackageSourcesArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct PackageSourcesArgs {
    pub packages_source: Option<PackagesSource>,
    pub package_mappings_source: Option<PackageMappingsSource>,
}

impl UiComponent for PackageSourcesModal {
    type UiContext<'context> = ();
    type UiCommand = PackageSourcesModalUiCommand;
    type UiAction = PackageSourcesModalAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        let mut file_picker = self.file_picker.lock().unwrap();
        if let Some((_reason, picker, command_fn)) = file_picker.as_mut() {
            // FIXME this `update` method does not get called immediately after picking a file, instead update gets
            //       called when the user moves the mouse or interacts with the window again.
            match picker.picked() {
                Ok(picked_file) => {
                    let command = command_fn(picked_file);
                    self.component.send(command);

                    *file_picker = None;
                }
                Err(PickError::Cancelled) => {
                    *file_picker = None;
                }
                _ => {}
            }
        }

        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("package_sources_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-package-sources-title", {file: file_name}));

            let form = Form::new(&self.fields, &self.component.sender, ());

            self.show_form(ui, &form);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-common-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(PackageSourcesModalUiCommand::Cancel);
                    }

                    if ui
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-common-button-ok")))
                        .clicked()
                    {
                        self.component
                            .send(PackageSourcesModalUiCommand::Submit);
                    }
                },
            );
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PackageSourcesModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = PackageSourcesArgs {
                    packages_source: fields.packages_source.clone(),
                    package_mappings_source: fields.package_mappings_source.clone(),
                };
                Some(PackageSourcesModalAction::Submit(args))
            }
            PackageSourcesModalUiCommand::Cancel => Some(PackageSourcesModalAction::CloseDialog),
            PackageSourcesModalUiCommand::PickPackagesSourceClicked => {
                self.pick_packages_file();
                None
            }
            PackageSourcesModalUiCommand::ClearPackagesSourceClicked => {
                self.fields
                    .lock()
                    .unwrap()
                    .packages_source = None;
                None
            }
            PackageSourcesModalUiCommand::PickPackageMappingsSourceClicked => {
                self.pick_package_mappings_file();
                None
            }
            PackageSourcesModalUiCommand::ClearPackageMappingsSourceClicked => {
                self.fields
                    .lock()
                    .unwrap()
                    .package_mappings_source = None;
                None
            }
            PackageSourcesModalUiCommand::PackagesSourcePicked(path) => {
                self.fields
                    .lock()
                    .unwrap()
                    .packages_source = Some(PackagesSource::File(path));
                None
            }
            PackageSourcesModalUiCommand::PackageMappingsSourcePicked(path) => {
                self.fields
                    .lock()
                    .unwrap()
                    .package_mappings_source = Some(PackageMappingsSource::File(path));
                None
            }
        }
    }
}
