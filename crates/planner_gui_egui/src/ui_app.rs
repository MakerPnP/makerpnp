use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use egui::{CentralPanel, ThemePreference};
use egui_i18n::tr;
use egui_mobius::slot::Slot;
use egui_mobius::types::{Enqueue, Value, ValueGuard};
use slotmap::SlotMap;
use tracing::{debug, info};

use crate::config::Config;
use crate::file_picker::Picker;
use crate::project::{Project, ProjectKey, ProjectUiCommand};
use crate::runtime::TokioRuntime;
use crate::tabs::TabKey;
use crate::toolbar::{Toolbar, ToolbarContext, ToolbarUiCommand};
use crate::ui_app::app_tabs::new_project::NewProjectArgs;
use crate::ui_app::app_tabs::project::{ProjectTab, ProjectTabUiCommand};
use crate::ui_app::app_tabs::{AppTabs, TabKind, TabKindContext, TabKindUiCommand, TabUiCommand};
use crate::ui_commands::{UiCommand, handle_command};
use crate::ui_component::{ComponentState, UiComponent};
use crate::{fonts, task};

pub mod app_tabs;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UiApp {
    app_tabs: Value<AppTabs>,

    config: Value<Config>,

    // state contains fields that cannot be initialized using 'Default'
    #[serde(skip)]
    state: MaybeUninit<Value<AppState>>,

    // the slot handler needs state, so slot can't be *in* state
    #[serde(skip)]
    slot: MaybeUninit<Slot<UiCommand>>,
}

pub struct AppState {
    // TODO find a better way of doing this that doesn't require this boolean
    startup_done: bool,
    file_picker: Picker,

    command_sender: Enqueue<UiCommand>,

    pub projects: Value<SlotMap<ProjectKey, Project>>,

    pub toolbar: Toolbar,
}

impl AppState {
    pub fn init(sender: Enqueue<UiCommand>) -> Self {
        let mut toolbar = Toolbar::new();
        toolbar
            .component
            .configure_mapper(sender.clone(), |command: ToolbarUiCommand| {
                debug!("app toolbar mapper. command: {:?}", command);
                UiCommand::ToolbarCommand(command)
            });

        Self {
            startup_done: false,
            file_picker: Picker::default(),

            command_sender: sender,
            projects: Value::new(SlotMap::default()),
            toolbar,
        }
    }

    pub fn pick_file(&mut self) {
        if !self.file_picker.is_picking() {
            self.file_picker.pick_file();
        }
    }

    pub fn make_project_tab(&mut self, path: PathBuf, project_key: ProjectKey) -> (TabKind, ProjectKey) {
        info!("open file. path: {:?}", path);

        let label = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let tab_kind_component = ComponentState::default();
        let tab_kind_sender = tab_kind_component.sender.clone();

        let mut project_tab = ProjectTab::new(label, path, project_key);
        project_tab
            .component
            .configure_mapper(tab_kind_sender, |command| {
                debug!("project tab mapper. command: {:?}", command);
                TabKindUiCommand::ProjectTabCommand {
                    command,
                }
            });

        let tab_kind = TabKind::Project(project_tab, tab_kind_component);

        (tab_kind, project_key)
    }

    pub fn send_project_command(
        command_sender: &mut Enqueue<UiCommand>,
        project_command: ProjectUiCommand,
        project_key: ProjectKey,
        tab_key: TabKey,
    ) {
        command_sender
            .send(UiCommand::TabCommand {
                tab_key,
                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                    command: ProjectTabUiCommand::ProjectCommand {
                        key: project_key,
                        command: project_command,
                    },
                }),
            })
            .expect("sent");
    }

    pub fn configure_project_tab(
        &mut self,
        project_key: ProjectKey,
        tab_key: TabKey,
        project_command: ProjectUiCommand,
    ) {
        let mut projects = self.projects.lock().unwrap();
        let mut project = projects.get_mut(project_key).unwrap();

        let app_command_sender = self.command_sender.clone();
        configure_project_component(app_command_sender, tab_key.clone(), &mut project);

        Self::send_project_command(&mut self.command_sender, project_command, project_key, tab_key);
    }

    pub fn open_file(&mut self, path: PathBuf, app_tabs: Value<AppTabs>) {
        let (project_command, project_key) = {
            let mut projects = self.projects.lock().unwrap();
            project_from_path(path.clone(), &mut projects)
        };

        let (tab_kind, project_key) = self.make_project_tab(path, project_key);

        let tab_key = app_tabs
            .lock()
            .unwrap()
            .add_tab(tab_kind);

        self.configure_project_tab(project_key, tab_key, project_command);
    }

    pub fn create_project(&mut self, tab_key: TabKey, args: NewProjectArgs, app_tabs: Value<AppTabs>) {
        debug!("creating project. tab_key: {:?}, args: {:?}", tab_key, args);

        let (project_command, project_key, path) = {
            let mut projects = self.projects.lock().unwrap();
            project_from_args(args, &mut projects)
        };

        let (tab_kind, project_key) = self.make_project_tab(path, project_key);

        app_tabs
            .lock()
            .unwrap()
            .replace(&tab_key, tab_kind)
            .expect("replaced");

        self.configure_project_tab(project_key, tab_key, project_command);
    }
}

impl Default for UiApp {
    fn default() -> Self {
        Self {
            app_tabs: Default::default(),
            config: Default::default(),
            state: MaybeUninit::uninit(),
            slot: MaybeUninit::uninit(),
        }
    }
}

impl UiApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        fonts::initialize(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut instance = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        {
            let config = instance.config.lock().unwrap();
            egui_i18n::set_language(&config.language_identifier);

            // Safety: now safe to use i18n translation system (e.g. [`egui_i18n::tr!`])
        }

        let (app_signal, mut app_slot) = egui_mobius::factory::create_signal_slot::<UiCommand>();

        let app_message_sender = app_signal.sender.clone();

        let runtime = TokioRuntime::new();

        let state = Value::new(AppState::init(app_message_sender.clone()));

        instance.state.write(state.clone());
        // Safety: `Self::state()` is now safe to call.

        let app_tabs = instance.app_tabs.clone();

        // Define a handler function for the slot
        let handler = {
            let app_tabs = app_tabs.clone();
            let config = instance.config.clone();
            let context = cc.egui_ctx.clone();

            move |command: UiCommand| {
                let task = handle_command(
                    command,
                    state.clone(),
                    app_tabs.clone(),
                    config.clone(),
                    context.clone(),
                );

                if let Some(future) = crate::task::into_future(task) {
                    runtime.runtime().spawn(future);
                }
            }
        };

        // Start the slot with the handler
        app_slot.start(handler);

        let mut app_tabs = app_tabs.lock().unwrap();
        app_tabs
            .component
            .configure_mapper(app_message_sender, |(tab_key, command)| {
                debug!("app tabs mapper. command: {:?}", command);
                UiCommand::TabCommand {
                    tab_key,
                    command,
                }
            });

        instance.slot.write(app_slot);

        instance
    }

    /// provide mutable access to the state.
    ///
    /// Safety: it's always safe, because `new` calls `state.write()`
    fn app_state(&mut self) -> ValueGuard<AppState> {
        unsafe {
            self.state
                .assume_init_mut()
                .lock()
                .unwrap()
        }
    }

    /// When the app starts up, the new project tab components won't be wired up, so we just
    /// remove them for now until we have a component restoration system.
    fn remove_new_project_tabs_on_startup(&mut self) {
        let mut ui_state = self.app_tabs.lock().unwrap();

        ui_state.retain(|_tab_key, tab_kind| !matches!(tab_kind, TabKind::NewProject(_, _)));
    }

    /// when the app starts up, the projects will be empty, and the document tabs will have keys that don't exist
    /// in the projects list (because it's empty now).
    /// we have to find these tabs, create projects, store them in the map and replace the tab's project key
    /// with the new key generated when adding the key to the map
    ///
    /// Safety: call only once on startup, before the tabs are shown.
    fn restore_documents_on_startup(&mut self) {
        // we have to do this as a two-step process to above borrow-checker issues
        // we also have to limit the scope of the access to ui_state and app_state

        // step 1 - find the document tabs, return the tab keys and paths.
        let tab_keys_and_paths = {
            let ui_state = self.app_tabs.lock().unwrap();

            ui_state.filter_map(|(tab_key, tab_kind)| match tab_kind {
                TabKind::Project(project_tab, _) => Some((tab_key.clone(), project_tab.path.clone())),
                _ => None,
            })
        };

        // step 2 - store the documents and update the document key for the tab.
        for (tab_key, path) in tab_keys_and_paths {
            let (project_key, project_command) = {
                let app_state = self.app_state();
                let app_command_sender = app_state.command_sender.clone();
                let mut projects = app_state.projects.lock().unwrap();

                let (project_command, project_key) = project_from_path(path, &mut projects);

                let mut project = projects.get_mut(project_key).unwrap();
                configure_project_component(app_command_sender, tab_key.clone(), &mut project);

                (project_key, project_command)
            };

            {
                let ui_state = self.app_tabs.lock().unwrap();
                ui_state.with_tab_mut(&tab_key, |tab| {
                    if let TabKind::Project(project_tab, _) = tab {
                        project_tab.project_key = project_key;
                    } else {
                        unreachable!()
                    }
                });
            }

            {
                let app_state = self.app_state();

                app_state
                    .command_sender
                    .send(UiCommand::TabCommand {
                        tab_key,
                        command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                            command: ProjectTabUiCommand::ProjectCommand {
                                key: project_key,
                                command: project_command,
                            },
                        }),
                    })
                    .expect("sent");
            }
        }
    }
}

impl eframe::App for UiApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        // NOTE: no File->Quit on web pages!
                        let is_web = cfg!(target_arch = "wasm32");
                        if !is_web {
                            ui.menu_button(tr!("menu-top-level-file"), |ui| {
                                if ui
                                    .button(tr!("menu-item-quit"))
                                    .clicked()
                                {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                            ui.add_space(16.0);
                        }
                    },
                    |ui| {
                        let theme_preference = ctx.options(|opt| opt.theme_preference);

                        egui::ComboBox::from_id_salt(ui.id().with("theme"))
                            .selected_text({
                                match theme_preference {
                                    ThemePreference::Dark => tr!("theme-button-dark"),
                                    ThemePreference::Light => tr!("theme-button-light"),
                                    ThemePreference::System => tr!("theme-button-system"),
                                }
                            })
                            .show_ui(ui, |ui| {
                                let sender = self.app_state().command_sender.clone();

                                if ui
                                    .add(egui::SelectableLabel::new(
                                        theme_preference.eq(&ThemePreference::Dark),
                                        tr!("theme-button-dark"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::Dark))
                                        .expect("sent");
                                }
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        theme_preference.eq(&ThemePreference::Light),
                                        tr!("theme-button-light"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::Light))
                                        .expect("sent");
                                }
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        theme_preference.eq(&ThemePreference::System),
                                        tr!("theme-button-system"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::System))
                                        .expect("sent");
                                }
                            });

                        let language = egui_i18n::get_language();
                        fn format_language_key(language_identifier: &String) -> String {
                            format!("language-{}", &language_identifier).to_string()
                        }

                        egui::ComboBox::from_id_salt(ui.id().with("language"))
                            .selected_text(tr!(&format_language_key(&language)))
                            .show_ui(ui, |ui| {
                                for other_language in egui_i18n::languages() {
                                    let sender = self.app_state().command_sender.clone();
                                    if ui
                                        .add(egui::SelectableLabel::new(
                                            other_language.eq(&language),
                                            tr!(&format_language_key(&other_language)),
                                        ))
                                        .clicked()
                                    {
                                        sender
                                            .send(UiCommand::LangageChanged(other_language.clone()))
                                            .expect("sent");
                                    }
                                }
                            });
                    },
                );
            });

            {
                let mut context = build_toolbar_context(&self.app_tabs);

                self.app_state()
                    .toolbar
                    .ui(ui, &mut context);
            }
        });

        if !self.app_state().startup_done {
            self.app_state().startup_done = true;

            {
                let mut ui_state = self.app_tabs.lock().unwrap();
                ui_state.show_home_tab_on_startup(
                    self.config
                        .lock()
                        .unwrap()
                        .show_home_tab_on_startup,
                );
            }
            self.remove_new_project_tabs_on_startup();
            self.restore_documents_on_startup();
        }

        // in a block to limit the scope of the `ui_state` borrow/guard
        {
            let projects = self.app_state().projects.clone();

            let mut tab_context = TabKindContext {
                config: self.config.clone(),
                projects,
            };

            let mut ui_state = self.app_tabs.lock().unwrap();

            // FIXME remove this when `on_close` bugs in egui_dock are fixed.
            ui_state.cleanup_tabs(&mut tab_context);

            CentralPanel::default().show(ctx, |ui| {
                ui_state.ui(ui, &mut tab_context);
            });
        }

        let mut app_state = self.app_state();

        if let Ok(picked_file) = app_state.file_picker.picked() {
            // FIXME this `update` method does not get called immediately after picking a file, instead update gets
            //       called when the user moves the mouse or interacts with the window again.
            app_state
                .command_sender
                .send(UiCommand::OpenFile(picked_file))
                .ok();
        }
    }
}

fn project_from_path(
    path: PathBuf,
    projects: &mut ValueGuard<SlotMap<ProjectKey, Project>>,
) -> (ProjectUiCommand, ProjectKey) {
    let mut project_command = None;
    let project_key = projects.insert_with_key(|key| {
        let (project, project_command_to_issue) = Project::from_path(path.clone(), key);
        project_command.replace(project_command_to_issue);

        project
    });
    (project_command.unwrap(), project_key)
}

fn project_from_args(
    args: NewProjectArgs,
    projects: &mut ValueGuard<SlotMap<ProjectKey, Project>>,
) -> (ProjectUiCommand, ProjectKey, PathBuf) {
    let path = args.build_path();

    let mut project_command = None;
    let project_key = projects.insert_with_key(|key| {
        let (project, project_command_to_issue) = Project::new(args.name, path.clone(), key);
        project_command.replace(project_command_to_issue);

        project
    });
    (project_command.unwrap(), project_key, path)
}

fn configure_project_component(app_command_sender: Sender<UiCommand>, tab_key: TabKey, project: &mut Project) {
    project
        .component
        .configure_mapper(app_command_sender, move |(key, command)| {
            debug!("project mapper. command: {:?}", command);
            UiCommand::TabCommand {
                tab_key,
                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                    command: ProjectTabUiCommand::ProjectCommand {
                        key,
                        command,
                    },
                }),
            }
        });
}

pub fn build_toolbar_context(app_tabs: &Value<AppTabs>) -> ToolbarContext {
    let app_tabs = app_tabs.lock().unwrap();
    let active_tab = app_tabs.active_tab();

    let can_save = active_tab.map_or(false, |tab_key| {
        app_tabs.with_tab_mut(&tab_key, |tab_kind| match tab_kind {
            TabKind::Home(_, _) => false,
            TabKind::NewProject(_, _) => false,
            TabKind::Project(project_tab, _) => project_tab.modified,
        })
    });

    let context = ToolbarContext {
        active_tab,
        can_save,
    };
    context
}
