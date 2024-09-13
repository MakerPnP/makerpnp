use tracing::trace;
use vizia::prelude::*;

pub trait TabbedDocument {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event);
    fn build_tab(&self) -> TabPair;
}

#[derive(Lens)]
pub struct TabbedDocumentContainer<T: TabbedDocument + 'static>
{
    tabs: Vec<T>,
}

impl<T: TabbedDocument + 'static> View for TabbedDocumentContainer<T> {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        trace!("event: {:?}", &event);
        for tab in self.tabs.iter_mut() {
            tab.event(cx, event);
        }
    }
}

impl<T: TabbedDocument + Clone + 'static> TabbedDocumentContainer<T> {
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
