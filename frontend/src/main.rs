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
    let theme = use_theme();

    html! {
        <>
            <Global css={css!(
                r#"
                    html, body {
                        font-family: sans-serif;
                        padding: 0;
                        margin: 0;

                        background-color: ${background_color};
                        color: ${text_color};
                    }

                    a, a:link, a:visited {
                        color: ${link_color};
                        text-decoration: none;
                    }

                    a:hover {
                        color: ${link_hover_color};
                    }

                    main {
                        padding: 1em;
                    }
                "#,
                background_color = &theme.colors.background,
                text_color = &theme.colors.text,
                link_color = &theme.colors.link,
                link_hover_color = &theme.colors.link_hover,
            )} />
            <BrowserRouter>
                <Navigation />
                <main>
                    <Switch<Route> render={Switch::render(switch)} />
                </main>
            </BrowserRouter>
        </>
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
