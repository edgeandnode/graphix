use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::*;
use crate::pages::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Overview,
    #[at("/poi-explorer")]
    POIExplorer,
    #[at("/poi-reports/deployment/:id")]
    POIExplorerDeployment { id: String },
    #[at("/poi-reports")]
    POIReports,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(routes: &Route) -> Html {
    match routes.clone() {
        Route::Overview | Route::POIExplorer => {
            html! {
                <Page title="POI Explorer">
                    <poi_explorer::Index />
                </Page>
            }
        }
        Route::POIExplorerDeployment { id } => {
            html! {
                <Page title={format!("POI Explorer: {}", &id)}>
                    <poi_explorer::Deployment id={id} />
                </Page>
            }
        }
        Route::POIReports => {
            html! { <div>{"POI Reports"}</div> }
        }
        Route::NotFound => {
            html! { <div>{"404 Not Found"}</div> }
        }
    }
}
