use vizia::prelude::{Color, Data, Element, Label, Localized, Percentage, ScrollView, TabPair};
use vizia::context::EventContext;
use vizia::events::Event;
use vizia::localization::ToStringLocalized;
use vizia::modifiers::{AbilityModifiers, LayoutModifiers, StyleModifiers};
use crate::project::{Project, ProjectContainer, ProjectRouteEvent};
use crate::route::Route;

#[derive(Clone, Data)]
pub struct ProjectTab {
    pub project: Option<Project>,
    pub route: Route,
}

impl ProjectTab {
    pub fn build_tab(&self, name: String) -> TabPair {
        let project = self.project.clone();

        let active_section = self.route.0.clone();

        let tab = TabPair::new(
            move |cx| {
                Label::new(cx, name.clone()).hoverable(false);
                Element::new(cx).class("indicator");
            },
            move |cx| {
                let project_for_scollview = project.clone();
                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    ProjectContainer::new(cx, project_for_scollview.clone(), active_section);
                })
                    .background_color(Color::rgb(0xdd, 0xdd, 0xdd))
                    .height(Percentage(100.0))
                    .width(Percentage(100.0));
            },
        );

        tab
    }

    pub fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|event, _meta| {
            println!("section event: {:?}", &event);
            match event {
                ProjectRouteEvent::RouteChanged { route } => {
                    self.route = route.clone()
                }
            }
        });
    }
}