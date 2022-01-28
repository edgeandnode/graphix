use stylist::yew::*;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::contexts::use_theme;
use crate::routes::*;

#[styled_component(Navigation)]
pub fn navigation() -> Html {
    let _theme = use_theme();

    html! {
        <header class={css!(
            r#"
                div {
                  display: flex;
                }

                a, a:link {
                  display: flex;
                  padding: 1em;
                }

                a:hover {
                  filter: brightness(70%);
                }
            "#,
        )}>
            <div>
                <Link<Route> to={Route::Overview}>{"Overview"}</Link<Route>>
                <Link<Route> to={Route::POIExplorer}>{"POI Explorer"}</Link<Route>>
                <Link<Route> to={Route::POIReports}>{"POI Reports"}</Link<Route>>
            </div>
        </header>
    }
}
