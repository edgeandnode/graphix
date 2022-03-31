use stylist::yew::*;
use yew::prelude::*;

#[derive(Clone, Properties, PartialEq)]
pub struct NavigationMenuItemProps {
    pub name: String,
    pub fontawesome_icon: String,
    pub is_active: bool,
}

#[styled_component(NavigationMenuItem)]
pub fn navigation_menu_item(props: &NavigationMenuItemProps) -> Html {
    let fontawesome_attrs = format!("fa mr-4 fa-{}", props.fontawesome_icon);
    let item_attrs = if props.is_active {
        "mb-2 bg-gray-800 rounded shadow"
    } else {
        "mb-2 rounded hover:shadow hover:bg-gray-800"
    };

    html! {
        <li class={item_attrs}>
            <a class="inline-block w-full h-full px-3 py-2 font-bold text-white">
                <i class={fontawesome_attrs}></i>
                <span>{props.name.as_str()}</span>
            </a>
        </li>
    }
}
