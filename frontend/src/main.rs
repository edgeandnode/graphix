pub mod components;
mod contexts;
pub mod pages;
mod routes;

use stylist::yew::*;
use yew::prelude::*;
use yew_router::prelude::*;

use components::*;
use contexts::*;
use routes::*;

#[styled_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <main>
                <div class="flex h-screen">
                    <Navigation />
                    <div class="lg:w-3/4 p-5">
                        <Switch<Route> render={Switch::render(switch)} />
                    </div>
                </div>
            </main>
        </BrowserRouter>
    }
}

#[styled_component(Root)]
fn root() -> Html {
    html! {
        <ThemeProvider>
            <App />
        </ThemeProvider>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("Starting up");

    yew::start_app::<Root>();
}
