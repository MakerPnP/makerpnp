use std::path::PathBuf;
use vizia::context::EventContext;
use vizia::events::Event;
use vizia::prelude::*;
use crate::ApplicationEvent;
use crate::popups::PopupWindow;

enum NewProjectPopupEvent {
    SetName { text: String },
    SetPath { text: String },
    Ok,
    Cancel,
}

#[derive(Clone, Data, Default, Debug, Lens)]
pub struct NewProjectPopup {
    pub name: String,
    pub path: String,
}

impl NewProjectPopup {

    pub fn on_event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        event.take(|event, _| match event {
            NewProjectPopupEvent::SetName { text } => self.name = text,
            NewProjectPopupEvent::SetPath { text } => self.path = text,

            NewProjectPopupEvent::Cancel => {
                ecx.emit(ApplicationEvent::PopupClosed {})
            }
            NewProjectPopupEvent::Ok => {
                ecx.emit(ApplicationEvent::PopupClosed {});
                ecx.emit(ApplicationEvent::CreateProject { name: self.name.clone(), path: PathBuf::from(&self.path) });
            }
        });
    }

    pub fn build<'a, L: Lens<Target = Option<PopupWindow>>>(&self, cx: &'a mut Context, lens: L) -> Handle<'a, Window> {
        Window::popup(cx, true, |cx| {
            VStack::new(cx, |cx: &mut Context| {
                let kind_lens = lens.map_ref(|optional_kind| {
                    match optional_kind {
                        Some(PopupWindow::NewProject(kind)) => kind,
                        _ => unreachable!()
                    }
                });

                let name_lens = kind_lens.then(NewProjectPopup::name);
                let path_lens = kind_lens.then(NewProjectPopup::path);

                HStack::new(cx, |cx|{
                    Label::new(cx, Localized::new("popup-new-project-name-label"))
                        .width(Stretch(1.0));
                    Textbox::new(cx, name_lens)
                        .width(Stretch(4.0))
                        .on_edit(|ecx, text| ecx.emit(NewProjectPopupEvent::SetName { text }));

                })
                    .width(Stretch(1.0));

                HStack::new(cx, |cx|{
                    Label::new(cx, Localized::new("popup-new-project-path-label"))
                        .width(Stretch(1.0));
                    Textbox::new(cx, path_lens)
                        .width(Stretch(4.0))
                        .on_edit(|ecx, text| ecx.emit(NewProjectPopupEvent::SetPath { text }));
                })
                    .width(Stretch(1.0));


                HStack::new(cx, |cx|{
                    Element::new(cx)
                        .width(Stretch(2.0));
                    Button::new(cx, |cx|Label::new(cx, "Cancel")) // TODO i18n
                        .on_press(|ecx| ecx.emit(NewProjectPopupEvent::Cancel))
                        .width(Stretch(0.95));
                    Element::new(cx)
                        .width(Stretch(0.1));
                    Button::new(cx, |cx|Label::new(cx, "Ok")) // TODO i18n
                        .on_press(|ecx| ecx.emit(NewProjectPopupEvent::Ok))
                        .width(Stretch(0.95));
                })
                    .width(Stretch(1.0));
            })
                .child_space(Pixels(20.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .row_between(Pixels(12.0));
        })
            .on_close(|cx| {
                cx.emit(NewProjectPopupEvent::Cancel);
            })
            .title(Localized::new("popup-new-project-title"))
            .inner_size((400, 200))
            .position((500, 100))
    }
}
