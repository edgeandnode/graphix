use std::ops::Deref;

use css_colors::{rgba, Color, Ratio};
use stylist::yew::styled_component;
use yew::html::ImplicitClone;
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColors {
    pub background: String,
    pub text: String,
    pub one: String,
    pub two: String,
    pub three: String,
    pub four: String,
    pub five: String,
    pub link: String,
    pub link_hover: String,
    pub ok: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub colors: ThemeColors,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: ThemeColors {
                background: "white".into(),
                text: "black".into(),
                one: "#1f2041ff".into(),
                two: "#4b3f72ff".into(),
                three: "#ffc857ff".into(),
                four: "#119da4ff".into(),
                five: "#19647eff".into(),
                link: "black".into(),
                link_hover: "#19647eff".into(),
                ok: rgba(0, 255, 0, 1.0)
                    .lighten(Ratio::from_percentage(20))
                    .to_css(),
                error: rgba(255, 0, 0, 1.0)
                    .lighten(Ratio::from_percentage(20))
                    .to_css(),
            },
        }
    }
}

impl ImplicitClone for Theme {}

#[derive(Debug, Clone)]
pub struct ThemeContext {
    inner: UseStateHandle<Theme>,
}

impl ThemeContext {
    pub fn new(inner: UseStateHandle<Theme>) -> Self {
        Self { inner }
    }

    pub fn _set(&self, theme: Theme) {
        self.inner.set(theme)
    }

    pub fn _get(&self) -> Theme {
        (*self.inner).clone()
    }
}

impl Deref for ThemeContext {
    type Target = Theme;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl PartialEq for ThemeContext {
    fn eq(&self, rhs: &Self) -> bool {
        *self.inner == *rhs.inner
    }
}

#[derive(Debug, PartialEq, Properties)]
pub struct ThemeProviderProps {
    pub children: Children,
}

#[styled_component(ThemeProvider)]
pub fn theme_provider(props: &ThemeProviderProps) -> Html {
    let theme = use_state(Theme::default);
    let theme_ctx = ThemeContext::new(theme);

    html! {
        <ContextProvider<ThemeContext> context={theme_ctx}>
            {props.children.clone()}
        </ContextProvider<ThemeContext>>
    }
}

pub fn use_theme() -> ThemeContext {
    use_context::<ThemeContext>().unwrap()
}
