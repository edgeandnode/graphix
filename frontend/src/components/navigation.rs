use stylist::yew::*;
use yew::prelude::*;
use yew_router::prelude::*;

use super::NavigationMenuItem;
use crate::contexts::use_theme;
use crate::routes::*;

#[styled_component(Navigation)]
pub fn navigation() -> Html {
    let _theme = use_theme();

    html! {
        <div class="px-4 py-2 bg-gray-200 bg-indigo-600 lg:w-1/4">
            <svg xmlns="http://www.w3.org/2000/svg" class="inline w-8 h-8 text-white lg:hidden" fill="none"
                viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
            </svg>
            <div class="hidden lg:block">
                <div class="my-2 mb-6">
                    <h1 class="text-2xl font-bold text-white">{"Graph IXI Dashboard"}</h1>
                </div>
                <ul>
                    <Link<Route> to={Route::POIExplorer}>
                        <NavigationMenuItem
                            fontawesome_icon="fingerprint"
                            is_active={false}
                            name="POI explorer" />
                    </Link<Route>>
                    <Link<Route> to={Route::POIReports}>
                        <NavigationMenuItem
                            fontawesome_icon="bug"
                            is_active={false}
                            name="POI cross-checking reports" />
                    </Link<Route>>
                </ul>
            </div>
        </div>
    }
}
