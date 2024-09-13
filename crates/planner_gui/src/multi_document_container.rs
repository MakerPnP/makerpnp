use tracing::trace;
use vizia::prelude::*;
use crate::tabbed_ui::TabKind;

#[derive(Lens)]
pub struct MultiDocumentContainer {
    tabs: Vec<TabKind>,
}

impl View for MultiDocumentContainer {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        trace!("event: {:?}", &event);
        for tab in self.tabs.iter_mut() {
            tab.event(cx, event);
        }
    }
}

impl MultiDocumentContainer {
    pub fn new(cx: &mut Context, tabs: Vec<TabKind>) -> Handle<Self> {
        Self {
            tabs,
        }.build(cx, |cx| {
            TabView::new(cx, MultiDocumentContainer::tabs, |cx, tab_kind_lens| {
                tab_kind_lens.get(cx).build_tab()
            })
                .background_color(Color::lightgray())
                .width(Percentage(100.0))
                .height(Percentage(100.0));
        })
    }
}
