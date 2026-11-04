use std::path::PathBuf;

use egui::{Context, ThemePreference};
use planner_app::{ObjectPath, PcbSide, PlacementPositionUnit};
use tracing::{debug, trace};

use crate::config::Config;
use crate::pcb::{PcbAction, PcbUiCommand};
use crate::process::{ProcessAction, ProcessUiCommand};
use crate::project::{ProjectAction, ProjectUiCommand};
use crate::sync::Value;
use crate::tabs::TabKey;
use crate::task::Task;
use crate::toolbar::{ToolbarAction, ToolbarUiCommand};
use crate::ui_app::app_tabs::home::HomeTabAction;
use crate::ui_app::app_tabs::new_pcb::NewPcbTabAction;
use crate::ui_app::app_tabs::new_process::NewProcessTabAction;
use crate::ui_app::app_tabs::new_project::NewProjectTabAction;
use crate::ui_app::app_tabs::pcb::{PcbTabAction, PcbTabUiCommand};
use crate::ui_app::app_tabs::process::{ProcessTabAction, ProcessTabUiCommand};
use crate::ui_app::app_tabs::project::{ProjectTabAction, ProjectTabUiCommand};
use crate::ui_app::app_tabs::{
    AppTabs, TabAction, TabKind, TabKindAction, TabKindContext, TabKindUiCommand, TabUiCommand,
};
use crate::ui_app::{AppState, build_toolbar_context};
use crate::ui_component::UiComponent;

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ToolbarCommand(ToolbarUiCommand),
    OpenProjectFile(PathBuf),
    OpenPcbFile(PathBuf),
    OpenProcessFile(PathBuf),
    TabCommand {
        tab_key: TabKey,
        command: TabUiCommand,
    },
    LangageChanged(String),
    ThemeChanged(ThemePreference),
    ShowPcb(PathBuf),
    ShowProcess(PathBuf),
    LocateComponent {
        pcb_file: PathBuf,
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
}

// TODO perhaps the return type of this method be `Task<Result<UiCommand, UiAppError>>`
pub fn handle_command(
    command: UiCommand,
    app_state: Value<AppState>,
    app_tabs: Value<AppTabs>,
    config: Value<Config>,
    ui_context: Context,
) -> Task<UiCommand> {
    trace!("Handling command: {:?}", command);

    match command {
        UiCommand::None => Task::none(),
        UiCommand::LangageChanged(language) => {
            egui_i18n::set_language(&language);
            config
                .lock()
                .unwrap()
                .language_identifier = language;
            Task::none()
        }
        UiCommand::ThemeChanged(theme) => {
            ui_context.set_theme(theme);
            Task::none()
        }
        UiCommand::ToolbarCommand(command) => {
            let mut context = build_toolbar_context(&app_tabs);

            let toolbar_action = app_state
                .lock()
                .unwrap()
                .toolbar
                .update(command, &mut context);

            let task = handle_toolbar_action(toolbar_action, &app_state, &app_tabs);
            task
        }
        UiCommand::OpenProjectFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_project_file(picked_file, app_tabs);
            Task::none()
        }
        UiCommand::OpenPcbFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_pcb_file(picked_file, app_tabs);
            Task::none()
        }
        UiCommand::OpenProcessFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_process_file(picked_file, app_tabs);
            Task::none()
        }
        UiCommand::ShowProcess(path) => {
            if let Ok(tab_key) = app_tabs
                .lock()
                .unwrap()
                .show_process_tab(&path)
            {
                debug!("showing process tab, tab_key: {:?}", tab_key);
                Task::none()
            } else {
                Task::done(UiCommand::OpenProcessFile(path))
            }
        }
        UiCommand::ShowPcb(path) => {
            if let Ok(tab_key) = app_tabs
                .lock()
                .unwrap()
                .show_pcb_tab(&path)
            {
                debug!("showing pcb tab, tab_key: {:?}", tab_key);
                Task::none()
            } else {
                Task::done(UiCommand::OpenPcbFile(path))
            }
        }
        UiCommand::LocateComponent {
            pcb_file,
            object_path,
            pcb_side,
            design_position,
            unit_position,
        } => {
            let app_state = app_state.lock().unwrap();

            let pcbs = app_state.pcbs.lock().unwrap();
            let pcb = pcbs
                .iter()
                .find(|(_candidate_key, candidate_pcb)| candidate_pcb.path().eq(&pcb_file));

            if let Some((pcb_key, pcb)) = pcb {
                pcb.component
                    .send((pcb_key, PcbUiCommand::LocateComponent {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }))
            }

            Task::none()
        }

        UiCommand::TabCommand {
            tab_key,
            command,
        } => {
            // block required limit the scope of the `app_state` guard
            let (projects, pcbs, processes) = {
                let app_state = app_state.lock().unwrap();
                let projects = app_state.projects.clone();
                let pcbs = app_state.pcbs.clone();
                let processes = app_state.processes.clone();
                drop(app_state);
                (projects, pcbs, processes)
            };

            let mut tab_context = TabKindContext {
                config,
                projects,
                pcbs,
                processes,
            };

            let action = {
                let mut app_tabs = app_tabs.lock().unwrap();
                app_tabs
                    .update((tab_key, command), &mut tab_context)
                    .inspect(|action| debug!("handling tab command action: {:?}", action))
            };
            match action {
                None => Task::none(),
                Some(TabAction::None) => Task::none(),
                Some(TabAction::TabKindAction {
                    action,
                }) => match action {
                    TabKindAction::None => Task::none(),
                    TabKindAction::HomeTabAction {
                        action,
                    } => match action {
                        HomeTabAction::None => Task::none(),
                    },
                    TabKindAction::NewProjectTabAction {
                        action,
                    } => match action {
                        NewProjectTabAction::Submit(args) => {
                            let mut app_state = app_state.lock().unwrap();
                            app_state.create_project(tab_key, args, app_tabs);
                            Task::none()
                        }
                    },
                    TabKindAction::NewPcbTabAction {
                        action,
                    } => match action {
                        NewPcbTabAction::Submit(args) => {
                            let mut app_state = app_state.lock().unwrap();
                            app_state.create_pcb(tab_key, args, app_tabs);
                            Task::none()
                        }
                    },
                    TabKindAction::PcbTabAction {
                        action,
                    } => match action {
                        PcbTabAction::PcbTask(key, task) => task.map(move |action| {
                            trace!("mapping project action: {:?}", action);
                            match action {
                                // map it to the corresponding UiCommand::TabCommand
                                PcbAction::UiCommand(command) => UiCommand::TabCommand {
                                    tab_key,
                                    command: TabUiCommand::TabKindCommand(TabKindUiCommand::PcbTabCommand {
                                        command: PcbTabUiCommand::PcbCommand {
                                            key,
                                            command,
                                        },
                                    }),
                                },
                                _ => panic!("unsupported"),
                            }
                        }),
                        PcbTabAction::SetModifiedState(modified_state) => {
                            let app_tabs = app_tabs.lock().unwrap();
                            app_tabs.with_tab_mut(&tab_key, |tab| match tab {
                                TabKind::Pcb(pcb_tab, _) => {
                                    pcb_tab.modified = modified_state;
                                }
                                _ => unreachable!(),
                            });
                            Task::none()
                        }
                        PcbTabAction::RequestRepaint => {
                            ui_context.request_repaint();
                            Task::none()
                        }
                    },
                    TabKindAction::NewProcessTabAction {
                        action,
                    } => match action {
                        NewProcessTabAction::Submit(args) => {
                            let mut app_state = app_state.lock().unwrap();
                            app_state.create_process(tab_key, args, app_tabs);
                            Task::none()
                        }
                    },
                    TabKindAction::ProcessTabAction {
                        action,
                    } => match action {
                        ProcessTabAction::ProcessTask(key, task) => task.map(move |action| match action {
                            // map it to the corresponding UiCommand::TabCommand
                            ProcessAction::UiCommand(command) => UiCommand::TabCommand {
                                tab_key,
                                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProcessTabCommand {
                                    command: ProcessTabUiCommand::ProcessCommand {
                                        key,
                                        command,
                                    },
                                }),
                            },
                            _ => panic!("unsupported"),
                        }),
                        ProcessTabAction::SetModifiedState(modified_state) => {
                            let app_tabs = app_tabs.lock().unwrap();
                            app_tabs.with_tab_mut(&tab_key, |tab| match tab {
                                TabKind::Process(process_tab, _) => {
                                    process_tab.modified = modified_state;
                                }
                                _ => unreachable!(),
                            });
                            Task::none()
                        }
                    },
                    TabKindAction::ProjectTabAction {
                        action,
                    } => match action {
                        ProjectTabAction::ProjectTask(key, task) => task.map(move |action| {
                            trace!("mapping project action: {:?}", action);
                            match action {
                                // map it to the corresponding UiCommand::TabCommand
                                ProjectAction::UiCommand(command) => UiCommand::TabCommand {
                                    tab_key,
                                    command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                                        command: ProjectTabUiCommand::ProjectCommand {
                                            key,
                                            command,
                                        },
                                    }),
                                },
                                _ => {
                                    // unsupported here, there is no corresponding TabCommand
                                    // should have already been handled by the project
                                    // HINT: when batching tasks, make sure the batch doesn't include ProjectAction::Task

                                    // Also can occur as follows:
                                    // BAD  = Some(Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcbUnitAssignments(pcb_index))))
                                    // GOOD = Some(ProjectAction::Task(key, Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcbUnitAssignments(pcb_index)))))

                                    panic!("unsupported");
                                }
                            }
                        }),
                        ProjectTabAction::SetModifiedState(modified_state) => {
                            let app_tabs = app_tabs.lock().unwrap();
                            app_tabs.with_tab_mut(&tab_key, |tab| match tab {
                                TabKind::Project(project_tab, _) => {
                                    project_tab.modified = modified_state;
                                }
                                _ => unreachable!(),
                            });
                            Task::none()
                        }
                        ProjectTabAction::RequestRepaint => {
                            ui_context.request_repaint();
                            Task::none()
                        }
                        ProjectTabAction::ShowPcb(path) => Task::done(UiCommand::ShowPcb(path)),
                        ProjectTabAction::ShowProcess(path) => Task::done(UiCommand::ShowProcess(path)),
                        ProjectTabAction::LocateComponent {
                            pcb_file,
                            object_path,
                            pcb_side,
                            design_position,
                            unit_position,
                        } => Task::done(UiCommand::LocateComponent {
                            pcb_file,
                            object_path,
                            pcb_side,
                            design_position,
                            unit_position,
                        }),
                    },
                },
            }
        }
    }
}

fn handle_toolbar_action(
    toolbar_action: Option<ToolbarAction>,
    app_state: &Value<AppState>,
    app_tabs: &Value<AppTabs>,
) -> Task<UiCommand> {
    let Some(toolbar_action) = toolbar_action else {
        return Task::none();
    };

    match toolbar_action {
        ToolbarAction::ShowHomeTab => {
            let mut app_tabs = app_tabs.lock().unwrap();
            app_tabs.show_home_tab();
            Task::none()
        }
        ToolbarAction::AddNewProjectTab => {
            let file_dialogs = app_state.lock().unwrap().file_dialogs();
            let mut app_tabs = app_tabs.lock().unwrap();
            app_tabs.add_new_project_tab(file_dialogs);
            Task::none()
        }
        ToolbarAction::AddNewPcbTab => {
            let file_dialogs = app_state.lock().unwrap().file_dialogs();
            let mut app_tabs = app_tabs.lock().unwrap();
            app_tabs.add_new_pcb_tab(file_dialogs);
            Task::none()
        }
        ToolbarAction::AddNewProcessTab => {
            let file_dialogs = app_state.lock().unwrap().file_dialogs();
            let mut app_tabs = app_tabs.lock().unwrap();
            app_tabs.add_new_process_tab(file_dialogs);
            Task::none()
        }
        ToolbarAction::CloseAllTabs => {
            let mut app_tabs = app_tabs.lock().unwrap();
            app_tabs.close_all_tabs();
            Task::none()
        }
        ToolbarAction::PickProjectFile => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_project_file();
            Task::none()
        }
        ToolbarAction::PickPcbFile => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_pcb_file();
            Task::none()
        }
        ToolbarAction::PickProcessFile => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_process_file();
            Task::none()
        }
        ToolbarAction::SaveTab(tab_key) => {
            let app_tabs = app_tabs.lock().unwrap();
            app_tabs.with_tab_mut(&tab_key, |tab_kind| match tab_kind {
                TabKind::Project(project_tab, _) => {
                    let command = UiCommand::TabCommand {
                        tab_key,
                        command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                            command: ProjectTabUiCommand::ProjectCommand {
                                key: project_tab.project_key,
                                command: ProjectUiCommand::Save,
                            },
                        }),
                    };
                    Task::done(command)
                }
                TabKind::Pcb(pcb_tab, _) => {
                    let command = UiCommand::TabCommand {
                        tab_key,
                        command: TabUiCommand::TabKindCommand(TabKindUiCommand::PcbTabCommand {
                            command: PcbTabUiCommand::PcbCommand {
                                key: pcb_tab.pcb_key,
                                command: PcbUiCommand::Save,
                            },
                        }),
                    };
                    Task::done(command)
                }
                TabKind::Process(process_tab, _) => {
                    let command = UiCommand::TabCommand {
                        tab_key,
                        command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProcessTabCommand {
                            command: ProcessTabUiCommand::ProcessCommand {
                                key: process_tab.process_key,
                                command: ProcessUiCommand::Save,
                            },
                        }),
                    };
                    Task::done(command)
                }
                _ => Task::none(),
            })
        }
    }
}
