use stylist::yew::*;
use yew::prelude::*;

use crate::components::PageHeader;

#[derive(Properties, PartialEq)]
pub struct PageProps {
    pub title: String,
    #[prop_or_default]
    pub children: Children,
}

#[styled_component(Page)]
pub fn page(props: &PageProps) -> Html {
    html! {
        <div>
            <PageHeader title={props.title.clone()} />
            <>
                { for props.children.iter() }
            </>
        </div>
    }
}
