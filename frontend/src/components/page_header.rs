use stylist::yew::*;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PageHeaderProps {
    pub title: String,
}

#[styled_component(PageHeader)]
pub fn page_header(props: &PageHeaderProps) -> Html {
    html! {
        <div class={css!(
            r#"
                h1 {
                  margin: 0 0 1rem 0;
                }
            "#,
        )}>
            <h1>{&props.title}</h1>
        </div>
    }
}
