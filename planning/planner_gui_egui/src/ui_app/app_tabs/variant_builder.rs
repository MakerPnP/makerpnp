//! A design variant's tab, which shows a variant builder: for a new one, first the form to choose its EDA tool and
//! placements file, and name its design and variant, then the variant builder's panels: its explorer, and its
//! "Overview", whose data sources are chosen, in groups, then saved, and built, and a panel for each data source and
//! output (`VBGUI`).

use std::collections::BTreeMap;
use std::mem::discriminant;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use derivative::Derivative;
use egui::{Button, TextEdit, Ui, Widget, WidgetText};
use egui_dock::{DockArea, Style, TabViewer};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_ltreeview::TreeViewState;
use serde::{Deserialize, Serialize};
use stores_app::{Source, TableKind, TableRow};
use tracing::debug;
use validator::Validate;
use variantbuilder_app::variant_builders::{
    DEFAULT_VARIANT_NAME, SubstitutionsSource, design_name, file_name, placements_extensions, source_path, stored_path,
};
use variantbuilder_app::{
    EdaTool, Event, NewVariantBuilder, VariantBuilderChange, VariantBuilderFile, VariantBuilderStatus,
    VariantBuilderView,
};

use crate::data_sources::panel::{DataSourcePanel, DataSourcePanelUiCommand};
use crate::data_sources::table::new_row;
use crate::file_picker::{FileDialogs, FileFilter, Picker, SaveFile};
use crate::forms::fields::{FieldGrid, FieldRows, PickerAction, picker};
use crate::forms::{Form, NamedResponse};
use crate::stores_core::StoresCoreService;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::variant_builder_core::VariantBuilderCoreService;

mod explorer;
mod panels;

use panels::Panels;
pub use panels::{DataSource, Output, Panel};

/// A field of a variant builder whose value is a file the user chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileField {
    Parts,
    PartMappings,
    AssemblyRules,
}

/// A field of a variant builder whose value is a list of files, to which those chosen are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilesField {
    GlobalSubstitutions,
    DesignSubstitutions,
}

/// A field of a variant builder that's a list of ref-des, typed separated by commas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ListField {
    RefDesList,
    RefDesExclusionList,
    RefDesNoPlaceList,
}

/// What a file dialog the tab opened is choosing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Picking {
    /// The new variant builder's placements.
    Placements,
    File(FileField),
    Files(FilesField),
    /// A new assembly rules file, to create.
    NewAssemblyRules,
}

/// The tab is persisted, so that it can be shown again when the application restarts.
#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct VariantBuilderTab {
    /// The new variant builder's EDA tool, placements, and names, until "Ok" creates it.
    fields: Value<NewVariantBuilderFields>,

    /// The variant builder's file, once it's created or opened.
    pub path: Option<PathBuf>,

    /// The changes made to the variant builder that are to be saved, at most one for each field, applied to what its
    /// file says to show it (`VBGUI-3`). Kept once saved, when they're what the file says, so that a change the core
    /// refuses isn't lost (`FORM-5`).
    #[serde(skip)]
    pending: Vec<VariantBuilderChange>,

    /// The ref-des lists as typed, so that a comma being typed isn't removed as the list is read.
    #[serde(skip)]
    list_texts: BTreeMap<ListField, String>,

    /// What the file dialog that's open is choosing.
    #[serde(skip)]
    picking: Option<Picking>,

    #[serde(skip)]
    pub component: ComponentState<VariantBuilderTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,

    /// The variant builder's panels, kept as they're arranged, so they're shown again when the application restarts
    /// (`VBGUI-9`).
    #[serde(default)]
    panels: Value<Panels>,

    /// The explorer's tree, which nodes are open and selected.
    #[serde(skip)]
    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    /// The data source panel's state, of every table the tab's panels show: the tables it has had the stores core
    /// open, and each changeable table's cell being edited, and its rows selected (`VBGUI-11`, `VBGUI-13`).
    #[serde(skip)]
    #[derivative(Debug = "ignore")]
    data_source_panel: DataSourcePanel,
}

#[derive(Debug, Clone)]
pub enum VariantBuilderTabUiCommand {
    EdaToolChanged(Option<EdaTool>),
    ClearPlacementsClicked,
    PickPlacementsClicked,
    PlacementsPicked(PathBuf),
    DesignNameChanged(String),
    VariantNameChanged(String),
    Submit,

    Changed(VariantBuilderChange),
    ListChanged(ListField, String),
    ClearClicked(FileField),
    PickClicked(FileField),
    FilePicked(FileField, PathBuf),
    AddClicked(FilesField),
    FilesPicked(FilesField, Vec<PathBuf>),
    RemoveClicked(FilesField, usize),
    /// Moves the file at the index one place up the list, before the one before it.
    MoveUpClicked(FilesField, usize),
    /// Moves the file at the index one place down the list, after the one after it.
    MoveDownClicked(FilesField, usize),
    NewAssemblyRulesClicked,
    NewAssemblyRulesPicked(PathBuf),
    SaveClicked,
    BuildClicked,

    /// Shows the panel, opening it if it isn't open, as the explorer's node was double-clicked (`VBGUI-10`).
    ShowPanel(Panel),

    // internal
    /// Opens the variant builder, reading its file.
    Open,
    /// A data source panel's command, such as opening its table, or a cell edited (`VBGUI-11`, `VBGUI-13`).
    Panel(DataSourcePanelUiCommand),
}

#[derive(Debug)]
pub enum VariantBuilderTabAction {
    /// Commands for this tab to handle next, in order.
    Task(Task<VariantBuilderTabUiCommand>),
}

pub struct VariantBuilderTabContext {
    pub tab_key: TabKey,
    pub variant_builders: Value<VariantBuilderCoreService>,
    /// The stores core, which shows the data sources and outputs (`VBGUI-11`).
    pub stores: Value<StoresCoreService>,
}

impl VariantBuilderTab {
    /// A tab for a new variant builder, or for the one whose file is given.
    pub fn new(file_dialogs: Arc<dyn FileDialogs>, path: Option<PathBuf>) -> Self {
        Self {
            path,
            file_picker: Value::new(Picker::new(file_dialogs)),
            ..Self::default()
        }
    }

    /// For a tab restored when the application starts, which has no file dialogs of its own.
    pub fn set_file_dialogs(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        self.file_picker = Value::new(Picker::new(file_dialogs));
    }

    /// What the core knows of the variant builder: its file, if it's open, and its status.
    fn known(
        &self,
        variant_builders: &Value<VariantBuilderCoreService>,
    ) -> (Option<VariantBuilderView>, VariantBuilderStatus) {
        let Some(path) = &self.path else {
            return (None, VariantBuilderStatus::default());
        };
        let mut view = variant_builders.lock().unwrap().view();
        (
            view.variant_builders.remove(path),
            view.statuses
                .remove(path)
                .unwrap_or_default(),
        )
    }

    /// What the variant builder's file says, with the changes to be saved.
    fn shown(&self, view: &VariantBuilderView) -> VariantBuilderFile {
        let mut file = view.file.clone();
        for change in self.pending.iter().cloned() {
            file.apply(change);
        }
        file
    }

    /// The files of the field, as shown, with the changes to be saved.
    fn shown_files(&self, variant_builders: &Value<VariantBuilderCoreService>, field: FilesField) -> Vec<PathBuf> {
        let (view, _) = self.known(variant_builders);
        view.map(|view| files(&self.shown(&view), field).clone())
            .unwrap_or_default()
    }

    /// Keeps the change to save, replacing any to the same field.
    fn change(&mut self, change: VariantBuilderChange) {
        self.pending
            .retain(|pending| discriminant(pending) != discriminant(&change));
        self.pending.push(change);
    }

    fn process(&self, variant_builders: &Value<VariantBuilderCoreService>, event: impl FnOnce(PathBuf) -> Event) {
        let Some(path) = &self.path else {
            return;
        };
        variant_builders
            .lock()
            .unwrap()
            .process(event(path.clone()));
    }

    /// The path to keep in the variant builder's file for a file chosen (`VARB-1`).
    fn stored(&self, path: &Path) -> PathBuf {
        match &self.path {
            Some(variant_builder) => stored_path(variant_builder, path),
            None => path.to_path_buf(),
        }
    }
}

impl Tab for VariantBuilderTab {
    type Context = VariantBuilderTabContext;

    fn label(&self) -> WidgetText {
        let label = self
            .path
            .as_ref()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| tr!("tab-label-new-design-variant"));

        WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        let mut tab_context = VariantBuilderTabContext {
            tab_key: *tab_key,
            variant_builders: context.variant_builders.clone(),
            stores: context.stores.clone(),
        };
        UiComponent::ui(self, ui, &mut tab_context);
    }
}

impl UiComponent for VariantBuilderTab {
    type UiContext<'context> = VariantBuilderTabContext;
    type UiCommand = VariantBuilderTabUiCommand;
    type UiAction = VariantBuilderTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let mut file_picker = self.file_picker.lock().unwrap();
        let command = match self.picking {
            None => None,
            Some(Picking::Files(field)) => file_picker
                .picked_multi()
                .ok()
                .map(|picked| VariantBuilderTabUiCommand::FilesPicked(field, picked)),
            Some(picking) => file_picker
                .picked()
                .ok()
                .map(|picked| match picking {
                    Picking::Placements => VariantBuilderTabUiCommand::PlacementsPicked(picked),
                    Picking::File(field) => VariantBuilderTabUiCommand::FilePicked(field, picked),
                    Picking::NewAssemblyRules => VariantBuilderTabUiCommand::NewAssemblyRulesPicked(picked),
                    Picking::Files(_) => unreachable!(),
                }),
        };
        drop(file_picker);
        if let Some(command) = command {
            self.component.send(command);
        }

        match &self.path {
            None => {
                let form = Form::new(&self.fields, &self.component.sender, ());
                self.show_form(ui, &form);
            }
            Some(path) => {
                let (view, status) = self.known(&context.variant_builders);
                let mut viewer = PanelViewer {
                    tab: self,
                    path,
                    view,
                    status,
                    stores: &context.stores,
                };
                let mut panels = self.panels.lock().unwrap();
                DockArea::new(&mut panels.0)
                    .id(ui.id().with("variant_builder_panels"))
                    .style(Style::from_egui(ui.ctx().global_style().as_ref()))
                    .show_inside(ui, &mut viewer);
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
        debug!("VariantBuilderTab command. command: {:?}", command);
        match command {
            VariantBuilderTabUiCommand::EdaToolChanged(eda_tool) => {
                let mut fields = self.fields.lock().unwrap();
                // `VBGUI-1`, the placements are chosen from the EDA tool's files, so they're chosen again.
                if fields.eda_tool != eda_tool {
                    fields.placements = None;
                }
                fields.eda_tool = eda_tool;
                None
            }
            VariantBuilderTabUiCommand::ClearPlacementsClicked => {
                self.fields.lock().unwrap().placements = None;
                None
            }
            VariantBuilderTabUiCommand::PickPlacementsClicked => {
                let Some(eda_tool) = self.fields.lock().unwrap().eda_tool else {
                    // the button is disabled until there's an EDA tool.
                    return None;
                };
                self.picking = Some(Picking::Placements);
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file_of(vec![FileFilter {
                        name: tr!("variant-builder-filter-placements"),
                        extensions: placements_extensions(eda_tool)
                            .iter()
                            .map(|extension| extension.to_string())
                            .collect(),
                    }]);
                None
            }
            VariantBuilderTabUiCommand::PlacementsPicked(placements) => {
                self.picking = None;
                let mut fields = self.fields.lock().unwrap();
                // `VBGUI-1`, the design is named after the placements, unless the user has named it.
                let named = fields
                    .placements
                    .as_deref()
                    .map(design_name)
                    .unwrap_or_default();
                if fields.design_name == named {
                    fields.design_name = design_name(&placements);
                }
                fields.placements = Some(placements);
                None
            }
            VariantBuilderTabUiCommand::DesignNameChanged(name) => {
                self.fields.lock().unwrap().design_name = name;
                None
            }
            VariantBuilderTabUiCommand::VariantNameChanged(name) => {
                self.fields.lock().unwrap().variant_name = name;
                None
            }
            VariantBuilderTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let (Some(eda_tool), Some(placements)) = (fields.eda_tool, &fields.placements) else {
                    // "Ok" is disabled until the form is valid.
                    return None;
                };
                let (design_name, variant_name) = (fields.design_name.trim(), fields.variant_name.trim());
                // `VBGUI-1`, `VARB-2`, in the placements' folder.
                let path = placements
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
                    .join(file_name(design_name, variant_name));
                let new = NewVariantBuilder {
                    design_name: Some(design_name.to_string()),
                    variant_name: Some(variant_name.to_string()),
                    changes: vec![
                        VariantBuilderChange::EdaTool(Some(eda_tool)),
                        VariantBuilderChange::Placements(Some(stored_path(&path, placements))),
                    ],
                };
                drop(fields);

                self.path = Some(path);
                // `VBGUI-9`
                self.panels
                    .lock()
                    .unwrap()
                    .ensure_layout();
                // `VBGUI-1`, an existing file isn't replaced.
                self.process(&context.variant_builders, |path| Event::Create {
                    path,
                    overwrite: false,
                    new,
                });
                None
            }
            VariantBuilderTabUiCommand::Open => {
                // `VBGUI-9`, those kept when the application last closed, or else the explorer and the "Overview".
                self.panels
                    .lock()
                    .unwrap()
                    .ensure_layout();
                self.process(&context.variant_builders, |path| Event::Open {
                    path,
                });
                None
            }
            VariantBuilderTabUiCommand::ShowPanel(panel) => {
                self.panels.lock().unwrap().show(panel);
                None
            }
            VariantBuilderTabUiCommand::Panel(command) => {
                self.data_source_panel
                    .update(command, &context.stores);
                None
            }
            VariantBuilderTabUiCommand::Changed(change) => {
                self.change(change);
                None
            }
            VariantBuilderTabUiCommand::ListChanged(field, text) => {
                let list = text
                    .split(',')
                    .map(str::trim)
                    .filter(|ref_des| !ref_des.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                self.list_texts.insert(field, text);
                self.change(match field {
                    ListField::RefDesList => VariantBuilderChange::RefDesList(list),
                    ListField::RefDesExclusionList => VariantBuilderChange::RefDesExclusionList(list),
                    ListField::RefDesNoPlaceList => VariantBuilderChange::RefDesNoPlaceList(list),
                });
                None
            }
            VariantBuilderTabUiCommand::ClearClicked(field) => {
                self.change(file_change(field, None));
                None
            }
            VariantBuilderTabUiCommand::PickClicked(field) => {
                self.picking = Some(Picking::File(field));
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file_of(csv_filters());
                None
            }
            VariantBuilderTabUiCommand::FilePicked(field, path) => {
                self.picking = None;
                self.change(file_change(field, Some(self.stored(&path))));
                None
            }
            VariantBuilderTabUiCommand::AddClicked(field) => {
                self.picking = Some(Picking::Files(field));
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_files_of(csv_filters());
                None
            }
            VariantBuilderTabUiCommand::FilesPicked(field, paths) => {
                self.picking = None;
                // added to those there are.
                let mut files = self.shown_files(&context.variant_builders, field);
                files.extend(
                    paths
                        .iter()
                        .map(|path| self.stored(path)),
                );
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::RemoveClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index < files.len() {
                    files.remove(index);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::MoveUpClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index > 0 && index < files.len() {
                    files.swap(index - 1, index);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::MoveDownClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index + 1 < files.len() {
                    files.swap(index, index + 1);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::NewAssemblyRulesClicked => {
                let (Some(path), (Some(view), _)) = (&self.path, self.known(&context.variant_builders)) else {
                    return None;
                };
                self.picking = Some(Picking::NewAssemblyRules);
                // `VBGUI-6`, named after the variant builder, in its folder.
                let file_name = format!(
                    "{}_{}_assembly_rules.csv",
                    view.file.design_name, view.file.variant_name
                );
                self.file_picker
                    .lock()
                    .unwrap()
                    .save_file(SaveFile {
                        directory: path.parent().map(Path::to_path_buf),
                        file_name,
                        filters: csv_filters(),
                    });
                None
            }
            VariantBuilderTabUiCommand::NewAssemblyRulesPicked(assembly_rules) => {
                self.picking = None;
                // `VBGUI-6`, the core sets the assembly rules, so a change to them not yet saved is replaced.
                self.pending
                    .retain(|change| !matches!(change, VariantBuilderChange::AssemblyRules(_)));
                let assembly_rules = self.stored(&assembly_rules);
                self.process(&context.variant_builders, |path| Event::CreateAssemblyRules {
                    path,
                    assembly_rules,
                });
                None
            }
            VariantBuilderTabUiCommand::SaveClicked => {
                // `VBGUI-3`, only the fields that differ from what the file says.
                let (Some(view), _) = self.known(&context.variant_builders) else {
                    return None;
                };
                let changes = self
                    .pending
                    .iter()
                    .filter(|change| {
                        let mut file = view.file.clone();
                        file.apply((*change).clone());
                        file != view.file
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                self.process(&context.variant_builders, |path| Event::Configure {
                    path,
                    changes,
                });
                None
            }
            VariantBuilderTabUiCommand::BuildClicked => {
                self.process(&context.variant_builders, |path| Event::Build {
                    path,
                });
                None
            }
        }
    }
}

/// The change that sets the field to the file, or none.
fn file_change(field: FileField, path: Option<PathBuf>) -> VariantBuilderChange {
    match field {
        FileField::Parts => VariantBuilderChange::Parts(path),
        FileField::PartMappings => VariantBuilderChange::PartMappings(path),
        FileField::AssemblyRules => VariantBuilderChange::AssemblyRules(path),
    }
}

/// The change that sets the field to the files.
fn files_change(field: FilesField, paths: Vec<PathBuf>) -> VariantBuilderChange {
    match field {
        FilesField::GlobalSubstitutions => VariantBuilderChange::GlobalSubstitutions(paths),
        FilesField::DesignSubstitutions => VariantBuilderChange::DesignSubstitutions(paths),
    }
}

/// The files of the field, as the variant builder's file says.
fn files(file: &VariantBuilderFile, field: FilesField) -> &Vec<PathBuf> {
    match field {
        FilesField::GlobalSubstitutions => &file.global_substitutions,
        FilesField::DesignSubstitutions => &file.design_substitutions,
    }
}

/// The data sources are CSV files.
fn csv_filters() -> Vec<FileFilter> {
    vec![FileFilter {
        name: tr!("variant-builder-filter-csv"),
        extensions: vec!["csv".to_string()],
    }]
}

impl VariantBuilderTab {
    /// The new design variant's form, its labels in one column, and its fields filling the rest of the width
    /// (`VBGUI-1`, `VBGUI-8`).
    fn show_form(&self, ui: &mut Ui, form: &Form<NewVariantBuilderFields, VariantBuilderTabUiCommand>) {
        let fields = self.fields.lock().unwrap().clone();
        let send = |command| self.component.send(command);

        let eda_label = tr!("variant-builder-input-eda-tool");
        let placements_label = tr!("variant-builder-input-placements");
        let design_name_label = tr!("variant-builder-input-design-name");
        let variant_name_label = tr!("variant-builder-input-variant-name");
        let label_width = FieldGrid::label_width(ui, &[
            eda_label.clone(),
            placements_label.clone(),
            design_name_label.clone(),
            variant_name_label.clone(),
        ]);

        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                FieldGrid::new(ui.id().with("new_design_variant"), label_width).show(ui, |rows| {
                    rows.row(&eda_label, |ui| {
                        egui::ComboBox::from_id_salt(ui.id().with("eda_tool"))
                            .width(ui.available_width())
                            .selected_text(eda_tool_label(fields.eda_tool))
                            .show_ui(ui, |ui| {
                                for eda_tool in [EdaTool::DipTrace, EdaTool::KiCad, EdaTool::EasyEda] {
                                    if ui
                                        .add(Button::selectable(
                                            fields.eda_tool == Some(eda_tool),
                                            eda_tool_label(Some(eda_tool)),
                                        ))
                                        .clicked()
                                    {
                                        send(VariantBuilderTabUiCommand::EdaToolChanged(Some(eda_tool)));
                                    }
                                }
                            })
                            .response
                            .named(eda_label.clone());
                    });
                    rows.errors(&form.field_error_messages("eda_tool"));

                    // `VBGUI-1`, the placements are chosen from the EDA tool's files, so once it's chosen.
                    if fields.eda_tool.is_some() {
                        let value = fields
                            .placements
                            .as_ref()
                            .map_or("".to_string(), |path| path.display().to_string());
                        match rows.picker(&placements_label, &value) {
                            Some(PickerAction::Clear) => send(VariantBuilderTabUiCommand::ClearPlacementsClicked),
                            Some(PickerAction::Choose) => send(VariantBuilderTabUiCommand::PickPlacementsClicked),
                            _ => {}
                        }
                        rows.errors(&form.field_error_messages("placements"));
                    }

                    for (field_name, label, value, command) in [
                        (
                            "design_name",
                            &design_name_label,
                            &fields.design_name,
                            VariantBuilderTabUiCommand::DesignNameChanged as fn(String) -> VariantBuilderTabUiCommand,
                        ),
                        (
                            "variant_name",
                            &variant_name_label,
                            &fields.variant_name,
                            VariantBuilderTabUiCommand::VariantNameChanged,
                        ),
                    ] {
                        rows.row(label, |ui| {
                            let mut text = value.clone();
                            TextEdit::singleline(&mut text)
                                .desired_width(ui.available_width())
                                .ui(ui)
                                .named(label.clone());
                            if *value != text {
                                send(command(text));
                            }
                        });
                        rows.errors(&form.field_error_messages(field_name));
                    }
                });

                ui.add_space(8.0);
                if ui
                    .add_enabled(form.is_valid(), Button::new(tr!("form-common-button-ok")))
                    .clicked()
                {
                    send(VariantBuilderTabUiCommand::Submit);
                }
            });
    }

    /// `view` is what the variant builder's file says, once it's read, and `status` what the core is doing with it,
    /// and how its last event went (`VARB-7`).
    fn show_variant_builder(
        &self,
        ui: &mut Ui,
        path: &Path,
        view: Option<VariantBuilderView>,
        status: VariantBuilderStatus,
    ) {
        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.label(tr!("variant-builder-path", { path: path.display().to_string() }));

                if let Some(error) = &status.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                let Some(view) = view else {
                    if status.busy {
                        ui.label(tr!("variant-builder-loading"));
                    }
                    return;
                };

                ui.separator();

                let file = self.shown(&view);
                self.show_fields(ui, &file);

                ui.separator();

                show_substitutions_applied(ui, &file);

                let unsaved = file != view.file;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(unsaved && !status.busy, Button::new(tr!("variant-builder-button-save")))
                        .clicked()
                    {
                        self.component
                            .send(VariantBuilderTabUiCommand::SaveClicked);
                    }
                    // `VBGUI-4`, a variant is built from what its file says, so only once the changes are saved.
                    if ui
                        .add_enabled(
                            !unsaved && !status.busy,
                            Button::new(tr!("variant-builder-button-build")),
                        )
                        .clicked()
                    {
                        self.component
                            .send(VariantBuilderTabUiCommand::BuildClicked);
                    }
                    if status.busy {
                        ui.spinner();
                    } else if status.built {
                        ui.label(tr!("variant-builder-built"));
                    }
                });
            });
    }

    /// The variant builder's fields: what it's of, which can't be changed, then the rest in groups (`VBGUI-2`), each
    /// group a grid whose labels are as wide as the widest of every group's, so every field is as wide as the others,
    /// filling the rest of the width (`VBGUI-8`).
    fn show_fields(&self, ui: &mut Ui, file: &VariantBuilderFile) {
        let send = |command| self.component.send(command);

        let eda_label = tr!("variant-builder-input-eda-tool");
        let placements_label = tr!("variant-builder-input-placements");
        let design_name_label = tr!("variant-builder-input-design-name");
        let variant_name_label = tr!("variant-builder-input-variant-name");
        let parts_label = tr!("variant-builder-input-parts");
        let part_mappings_label = tr!("variant-builder-input-part-mappings");
        let global_label = tr!("variant-builder-input-global-substitutions");
        let assembly_rules_label = tr!("variant-builder-input-assembly-rules");
        let design_label = tr!("variant-builder-input-design-substitutions");
        let lists = [
            (ListField::RefDesList, tr!("variant-builder-input-ref-des-list")),
            (
                ListField::RefDesExclusionList,
                tr!("variant-builder-input-ref-des-exclusion-list"),
            ),
            (
                ListField::RefDesNoPlaceList,
                tr!("variant-builder-input-ref-des-no-place-list"),
            ),
        ];
        let outputs = [
            (
                tr!("variant-builder-input-output-placements"),
                &file.output_placements,
                VariantBuilderChange::OutputPlacements as fn(Option<PathBuf>) -> VariantBuilderChange,
            ),
            (
                tr!("variant-builder-input-output-bom"),
                &file.output_bom,
                VariantBuilderChange::OutputBom,
            ),
        ];

        let mut labels = vec![
            eda_label.clone(),
            placements_label.clone(),
            design_name_label.clone(),
            variant_name_label.clone(),
            parts_label.clone(),
            part_mappings_label.clone(),
            global_label.clone(),
            assembly_rules_label.clone(),
            design_label.clone(),
        ];
        labels.extend(
            lists
                .iter()
                .map(|(_, label)| label.clone()),
        );
        labels.extend(
            outputs
                .iter()
                .map(|(label, ..)| label.clone()),
        );
        let label_width = FieldGrid::label_width(ui, &labels);

        let group = |ui: &mut Ui, id: &str, title: Option<String>, add_rows: &mut dyn FnMut(&mut FieldRows)| {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                if let Some(title) = title {
                    ui.strong(title);
                }
                FieldGrid::new(ui.id().with(id), label_width).show(ui, |rows| add_rows(rows));
            });
        };

        // `VBGUI-1`, set as it's created, and kept as they are.
        group(ui, "variant_builder_of", None, &mut |rows| {
            let placements = file
                .placements
                .as_ref()
                .map_or("".to_string(), |path| path.display().to_string());
            rows.read_only(&eda_label, &eda_tool_label(file.eda_tool));
            rows.read_only(&placements_label, &placements);
            rows.read_only(&design_name_label, &file.design_name);
            rows.read_only(&variant_name_label, &file.variant_name);
        });

        group(
            ui,
            "part_matching",
            Some(tr!("variant-builder-group-part-matching")),
            &mut |rows| {
                self.show_file(rows, FileField::Parts, &parts_label, &file.parts);
                self.show_file(rows, FileField::PartMappings, &part_mappings_label, &file.part_mappings);
                self.show_files(rows, file, FilesField::GlobalSubstitutions, &global_label);
            },
        );

        group(
            ui,
            "design_specific",
            Some(tr!("variant-builder-group-design-specific")),
            &mut |rows| {
                self.show_file(
                    rows,
                    FileField::AssemblyRules,
                    &assembly_rules_label,
                    &file.assembly_rules,
                );
                self.show_files(rows, file, FilesField::DesignSubstitutions, &design_label);
            },
        );

        group(
            ui,
            "reference_designators",
            Some(tr!("variant-builder-group-reference-designators")),
            &mut |rows| {
                for (field, label) in lists.iter() {
                    self.show_list(rows, file, *field, label);
                }
            },
        );

        group(ui, "output", Some(tr!("variant-builder-group-output")), &mut |rows| {
            // typed, relative to the variant builder's folder, or its defaults if they're empty.
            for (label, value, change) in outputs.iter() {
                rows.row(label, |ui| {
                    let mut text = value
                        .as_ref()
                        .map_or("".to_string(), |path| path.display().to_string());
                    if TextEdit::singleline(&mut text)
                        .hint_text(tr!("variant-builder-output-default"))
                        .desired_width(ui.available_width())
                        .ui(ui)
                        .named(label.clone())
                        .changed()
                    {
                        let path = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
                        send(VariantBuilderTabUiCommand::Changed(change(path)));
                    }
                });
            }
        });
    }

    /// A file field: the file shown, and buttons named after the field to clear it and to choose it, and, for the
    /// assembly rules, to create new ones (`VBGUI-6`).
    fn show_file(&self, rows: &mut FieldRows, field: FileField, label: &str, value: &Option<PathBuf>) {
        let value = value
            .as_ref()
            .map_or("".to_string(), |path| path.display().to_string());
        let others = match field {
            FileField::AssemblyRules => vec![tr!("variant-builder-button-new")],
            _ => vec![],
        };
        let action = rows.row(label, |ui| picker(ui, label, &value, &others));
        let command = match action {
            Some(PickerAction::Clear) => VariantBuilderTabUiCommand::ClearClicked(field),
            Some(PickerAction::Choose) => VariantBuilderTabUiCommand::PickClicked(field),
            Some(PickerAction::Other(_)) => VariantBuilderTabUiCommand::NewAssemblyRulesClicked,
            None => return,
        };
        self.component.send(command);
    }

    /// A list of files, in a table, a row for each file: its place in the list, its path, and buttons to move it up and
    /// down the list, and to remove it, named after the field and the file, lined up by the table, then a button to add
    /// files (`VBGUI-2`).
    fn show_files(&self, rows: &mut FieldRows, file: &VariantBuilderFile, field: FilesField, label: &str) {
        let paths = files(file, field);
        rows.row(label, |ui| {
            let row_height = ui.spacing().interact_size.y;
            TableBuilder::new(ui)
                .id_salt(field)
                .striped(true)
                .vscroll(false)
                .auto_shrink([false, true])
                .column(Column::auto())
                .column(Column::remainder())
                .column(Column::auto())
                .header(row_height, |mut header| {
                    for key in [
                        "variant-builder-column-order",
                        "variant-builder-column-file",
                        "variant-builder-column-actions",
                    ] {
                        header.col(|ui| {
                            ui.strong(tr!(key));
                        });
                    }
                })
                .body(|mut body| {
                    for (index, path) in paths.iter().enumerate() {
                        let path = path.display().to_string();
                        body.row(row_height, |mut row| {
                            row.col(|ui| {
                                ui.label((index + 1).to_string());
                            });
                            row.col(|ui| {
                                ui.label(&path);
                            });
                            row.col(|ui| {
                                ui.horizontal(|ui| self.show_file_actions(ui, field, label, &path, index, paths.len()));
                            });
                        });
                    }
                });
            if ui
                .button(tr!("variant-builder-button-add"))
                .named(format!("{}: {}", label, tr!("variant-builder-button-add")))
                .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::AddClicked(field));
            }
        });
    }

    /// The buttons to move the file at the index up and down its list, and to remove it, each named after the field,
    /// what it does, and the file.
    fn show_file_actions(&self, ui: &mut Ui, field: FilesField, label: &str, path: &str, index: usize, count: usize) {
        for (enabled, text, action, command) in [
            (
                index > 0,
                "⏶",
                tr!("variant-builder-button-move-up"),
                VariantBuilderTabUiCommand::MoveUpClicked(field, index),
            ),
            (
                index + 1 < count,
                "⏷",
                tr!("variant-builder-button-move-down"),
                VariantBuilderTabUiCommand::MoveDownClicked(field, index),
            ),
            (
                true,
                "x",
                tr!("variant-builder-button-remove"),
                VariantBuilderTabUiCommand::RemoveClicked(field, index),
            ),
        ] {
            if ui
                .add_enabled(enabled, Button::new(text))
                .named(format!("{}: {} {}", label, action, path))
                .clicked()
            {
                self.component.send(command);
            }
        }
    }

    /// A ref-des list, typed separated by commas.
    fn show_list(&self, rows: &mut FieldRows, file: &VariantBuilderFile, field: ListField, label: &str) {
        let list = match field {
            ListField::RefDesList => &file.ref_des_list,
            ListField::RefDesExclusionList => &file.ref_des_exclusion_list,
            ListField::RefDesNoPlaceList => &file.ref_des_no_place_list,
        };
        let mut text = self
            .list_texts
            .get(&field)
            .cloned()
            .unwrap_or_else(|| list.join(", "));
        rows.row(label, |ui| {
            if TextEdit::singleline(&mut text)
                .desired_width(ui.available_width())
                .ui(ui)
                .named(label.to_string())
                .changed()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::ListChanged(field, text));
            }
        });
    }
}

/// The substitutions, in a table, in the order the core applies them, with the changes not yet saved, so the user sees
/// the effect of the order of the design's and the global ones: each one's place, its path, and the list it's from
/// (`VBGUI-7`, `VARB-5`).
fn show_substitutions_applied(ui: &mut Ui, file: &VariantBuilderFile) {
    ui.strong(tr!("variant-builder-substitutions-applied"));
    let substitutions = file.substitutions().collect::<Vec<_>>();
    if substitutions.is_empty() {
        ui.label(tr!("variant-builder-substitutions-applied-none"));
    } else {
        let row_height = ui.spacing().interact_size.y;
        TableBuilder::new(ui)
            .id_salt("substitutions_applied")
            .striped(true)
            .vscroll(false)
            .auto_shrink([false, true])
            .column(Column::auto())
            .column(Column::remainder())
            .column(Column::auto())
            .header(row_height, |mut header| {
                for key in [
                    "variant-builder-column-order",
                    "variant-builder-column-file",
                    "variant-builder-column-source",
                ] {
                    header.col(|ui| {
                        ui.strong(tr!(key));
                    });
                }
            })
            .body(|mut body| {
                for (index, (source, path)) in substitutions.iter().enumerate() {
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label((index + 1).to_string());
                        });
                        row.col(|ui| {
                            ui.label(path.display().to_string());
                        });
                        row.col(|ui| {
                            ui.label(match source {
                                SubstitutionsSource::Design => tr!("variant-builder-source-design"),
                                SubstitutionsSource::Global => tr!("variant-builder-source-global"),
                            });
                        });
                    });
                }
            });
    }
    ui.separator();
}

/// Shows each of a design variant's panels (`VBGUI-9`), `view` being what the variant builder's file says, once it's
/// read, and `status` what the core is doing with it.
struct PanelViewer<'a> {
    tab: &'a VariantBuilderTab,
    path: &'a Path,
    view: Option<VariantBuilderView>,
    status: VariantBuilderStatus,
    stores: &'a Value<StoresCoreService>,
}

impl TabViewer for PanelViewer<'_> {
    type Tab = Panel;

    fn id(&mut self, panel: &mut Panel) -> egui::Id {
        egui::Id::new(("variant_builder_panel", &*panel))
    }

    fn title(&mut self, panel: &mut Panel) -> WidgetText {
        panel.label()
    }

    /// egui_dock's tab buttons have no accessibility information, without it they can't be found by screen readers or
    /// by the UI tests, as for the application's tabs.
    fn on_tab_button(&mut self, panel: &mut Panel, response: &egui::Response) {
        let title = panel.label().text().to_string();

        response
            .ctx
            .accesskit_node_builder(response.id, |node| {
                node.set_role(egui::accesskit::Role::Tab);
                node.set_label(title);
            });
    }

    /// The explorer is always shown, as the other panels are opened from it (`VBGUI-9`).
    fn is_closeable(&self, panel: &Panel) -> bool {
        !matches!(panel, Panel::Explorer)
    }

    /// A table scrolls itself, and takes all the panel's space (`VBGUI-11`), the others scroll down.
    fn scroll_bars(&self, panel: &Panel) -> [bool; 2] {
        match panel {
            Panel::Explorer | Panel::Overview | Panel::BuildOutput => [false, true],
            Panel::DataSource {
                ..
            }
            | Panel::Output(_) => [false, false],
        }
    }

    fn ui(&mut self, ui: &mut Ui, panel: &mut Panel) {
        match panel {
            Panel::Explorer => self.show_explorer(ui),
            Panel::Overview => self
                .tab
                .show_variant_builder(ui, self.path, self.view.clone(), self.status.clone()),
            Panel::DataSource {
                source,
                path,
            } => self.show_data_source(ui, *source, path),
            Panel::BuildOutput => {
                egui::Frame::new()
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        // `VBGUI-12`, the mapping results are to come.
                        ui.label(tr!("variant-builder-build-output-to-come"));
                    });
            }
            Panel::Output(output) => self.show_output(ui, *output),
        }
    }
}

impl PanelViewer<'_> {
    /// The explorer, of the variant builder as the "Overview" shows it, with the changes not yet saved (`VBGUI-10`).
    fn show_explorer(&self, ui: &mut Ui) {
        let Some(view) = &self.view else {
            ui.spinner();
            return;
        };
        let file = self.tab.shown(view);
        let name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut state = self.tab.tree_view_state.lock().unwrap();
        if let Some(panel) = explorer::show_explorer(ui, &name, &file, &mut state) {
            self.tab
                .component
                .send(VariantBuilderTabUiCommand::ShowPanel(panel));
        }
    }

    /// A data source, as a table, its path relative to the variant builder's folder (`VBGUI-11`, `VARB-9`).
    fn show_data_source(&self, ui: &mut Ui, source: DataSource, path: &Path) {
        let Some(view) = &self.view else {
            ui.spinner();
            return;
        };
        let data_source = Source::File(source_path(self.path, path));
        let kind = match source {
            DataSource::Parts => TableKind::Parts,
            DataSource::Placements => match view.file.eda_tool {
                Some(eda_tool) => TableKind::EdaPlacements {
                    eda_tool,
                },
                None => {
                    ui.label(tr!("variant-builder-no-eda-tool"));
                    return;
                }
            },
            DataSource::PartMappings => TableKind::PartMappings,
            DataSource::GlobalSubstitutions | DataSource::DesignSubstitutions => TableKind::Substitutions,
            DataSource::AssemblyRules => TableKind::AssemblyRules,
        };
        // `VBGUI-13`, `VBGUI-14`, a part mapping, or a substitution, added is of the variant builder's EDA tool, and
        // `VBGUI-15`, `VBGUI-16`, an assembly rule, or a part, which has none, is added whether or not it has one.
        let new_row = new_row(kind, view.file.eda_tool);
        self.show_table(ui, kind, data_source, new_row);
    }

    /// The variant's placements or BOM, where building writes them, as its file says (`VBGUI-12`, `VARB-9`).
    fn show_output(&self, ui: &mut Ui, output: Output) {
        let Some(view) = &self.view else {
            ui.spinner();
            return;
        };
        let outputs = view.file.outputs(self.path);
        let (kind, path) = match output {
            Output::Placements => (TableKind::Placements, outputs.placements),
            Output::Bom => (TableKind::Bom, outputs.bom),
        };
        self.show_table(ui, kind, Source::File(path), None);
    }

    /// The table, with the GUI's one data source panel, as a parts tab shows its parts (`VBGUI-11`, `PARTSRC-3`), which
    /// edits one whose kind can be changed, "Add row" adding `new_row` (`VBGUI-13`).
    fn show_table(&self, ui: &mut Ui, kind: TableKind, source: Source, new_row: Option<TableRow>) {
        let send = |command: DataSourcePanelUiCommand| {
            self.tab
                .component
                .send(VariantBuilderTabUiCommand::Panel(command))
        };
        self.tab
            .data_source_panel
            .show(ui, self.stores, kind, &source, new_row, &send);
    }
}

fn eda_tool_label(eda_tool: Option<EdaTool>) -> String {
    match eda_tool {
        None => tr!("form-common-combo-select"),
        Some(EdaTool::DipTrace) => "DipTrace".to_string(),
        Some(EdaTool::KiCad) => "KiCad".to_string(),
        Some(EdaTool::EasyEda) => "EasyEDA".to_string(),
    }
}

/// A new design variant: the EDA tool, then its placements, then the names of the design and the variant, which name
/// the variant builder's file (`VBGUI-1`).
#[derive(Clone, Debug, Validate, Serialize, Deserialize)]
struct NewVariantBuilderFields {
    #[validate(required(code = "form-option-error-required"))]
    eda_tool: Option<EdaTool>,

    #[validate(required(code = "form-option-error-required"))]
    placements: Option<PathBuf>,

    #[validate(length(min = 1, code = "form-input-error-length"))]
    design_name: String,

    #[validate(length(min = 1, code = "form-input-error-length"))]
    variant_name: String,
}

impl Default for NewVariantBuilderFields {
    fn default() -> Self {
        Self {
            eda_tool: None,
            placements: None,
            design_name: String::new(),
            variant_name: DEFAULT_VARIANT_NAME.to_string(),
        }
    }
}
