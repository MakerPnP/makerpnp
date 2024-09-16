use tracing::{info, trace};
use vizia::prelude::*;

pub trait TabbedDocument {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event);
    fn build_tab(&self) -> TabPair;
}

pub enum TabbedDocumentEvent<T: TabbedDocument + Send + 'static> {
    AddTab { 
        tab: T, 
    }
}

#[derive(Lens)]
pub struct TabbedDocumentContainer<T: TabbedDocument + 'static>
{
    tabs: Vec<T>,
}

impl<T: TabbedDocument + Send + 'static> View for TabbedDocumentContainer<T> {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        trace!("event: {:?}", &event);
        for tab in self.tabs.iter_mut() {
            tab.event(cx, event);
        }
        event.take(|event, _meta|{
            match event {
                TabbedDocumentEvent::<T>::AddTab { tab } => {
                    info!("AddTab");
                    self.tabs.push(tab);
                }
            }
        })
    }
}

impl<T: TabbedDocument + Send + Clone + 'static> TabbedDocumentContainer<T> {
    pub fn new(cx: &mut Context, tabs: Vec<T>) -> Handle<Self> {
        Self {
            tabs,
        }.build(cx, |cx| {
            TabView::new(cx, TabbedDocumentContainer::<T>::tabs, |cx, tab_kind_lens| {
                tab_kind_lens.get(cx).build_tab()
            })
                .background_color(Color::lightgray())
                .width(Percentage(100.0))
                .height(Percentage(100.0));
        })
    }
}
