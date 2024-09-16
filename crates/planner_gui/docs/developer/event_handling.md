# Event handling

! There are just some notes that were made during the course of development, they may be out-of-date

## Create project flow

App starts:
    Tab container emits InternalEvent::DocumentContainerCreated after builds.

AppData::event
* records the source entity from the InternalEvent::DocumentContainerCreated message.
  so that later, when can use this entity to create a new tab.

App creates binding using the AppData::popup_window lens.

User clicks button.
* Emit Event: ApplicationEvent::ShowCreateProject
AppData::event
* Gui creates popup window
* updates appdata lens for popup_window
Binding / Lens for AppData::popup_window activated.
* PopupWindow::build gets called
* delegates to NewProjectPopup::build

TODO: user enters 'name' and 'path'

Use clicks close button.
* ApplicationEvent::PopupClosed is emitted
AppData::event
* handles PopupClosed event, calls PopupWindow::on_close
* delegates to NewProjectPopup::on_close
* emits ApplicationEvent::CreateProject message, with name and path.

AppData::event
* handles CreateProject event
* raises CoreApp::CreateProject event, via CoreService::update, passed vizia event context

Planner::update
* handles CoreApp::CreateProject event
* calls Project::new()
* stores the resulting project in the app mode.
* emits CoreApp::CreatedProject(Ok(()))

Planner::update
* handles CoreApp::CreatedProject event
* creates a path to navigate to based on the model.
* uses 'navigate' capability to tell the GUI to load the project.

CoreService::update processes effects
* handles navigate effect.
* raises ApplicationEvent::OpenProject with the path of the newly created project.

AppData::event
* handles ApplicationEvent::OpenProject
* creates new tab
* emits TabbedDocumentEvent::AddTab with the new tab

TabbedDocumentContainer::event
* handles TabbedDocumentEvent::AddTab
* adds tab to Self::tabs

TabView uses TabbedDocumentContainer::<T>::tabs lens
* List of tabs updated.

