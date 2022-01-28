use anyhow;
use gloo::timers::callback::Interval;
use graphql_client::{GraphQLQuery, Response as GraphQLResponse};
use log::warn;
use reqwasm::http::*;
use stylist::{css, yew::*};
use wasm_bindgen_futures::spawn_local;
use yew::{html::Scope, prelude::*};
use yew_router::prelude::*;

use crate::routes::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "./graphql/api_schema.graphql",
    query_path = "./graphql/queries/deployments.graphql",
    response_derives = "Debug"
)]
struct Deployments;

async fn fetch_deployments() -> Result<Vec<String>, anyhow::Error> {
    let query = Deployments::build_query(deployments::Variables {});
    let query = serde_json::json!(query);

    let request = Request::new("http://localhost:3030/graphql")
        .body(query.to_string())
        .header("content-type", "application/json")
        .method(Method::POST)
        .mode(RequestMode::Cors);

    let response: GraphQLResponse<deployments::ResponseData> = request.send().await?.json().await?;

    match (response.data, response.errors) {
        (Some(data), _) => Ok(data.deployments),
        (_, Some(errors)) => {
            warn!("Errors fetching deployments: {:?}", errors);
            Ok(vec![])
        }
        (_, _) => unreachable!(),
    }
}

fn poll_deployments(link: Scope<View>) {
    spawn_local(async move {
        match fetch_deployments().await {
            Ok(deployments) => link.send_message(Msg::UpdateDeployments(deployments)),
            Err(error) => warn!("Failed to fetch deployments: {}", error),
        }
    });
}

#[derive(Properties, PartialEq)]
pub struct DeploymentLinkProps {
    pub deployment: String,
}

#[styled_component(DeploymentLink)]
pub fn deployment_link(props: &DeploymentLinkProps) -> Html {
    html! {
        <div>
           <Link<Route> to={Route::POIExplorerDeployment { id: props.deployment.clone() }}>{&props.deployment}</Link<Route>>
        </div>
    }
}

pub struct View {
    deployments: Vec<String>,
    _deployments_interval: Interval,
}

pub enum Msg {
    UpdateDeployments(Vec<String>),
}

impl Component for View {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();

        // Fetch deployments immediately
        poll_deployments(link.clone());

        Self {
            deployments: vec![],

            // Refetch deployments every 5s
            _deployments_interval: Interval::new(5000, move || poll_deployments(link.clone())),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::UpdateDeployments(deployments) => {
                self.deployments = deployments;
            }
        }
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class={css!("a, a:link { font-family: monospace; }")}>
            {
                for self.deployments.iter().map(|d| {
                    html!{ <DeploymentLink deployment={d.clone()} /> }
                })
            }
            </div>
        }
    }
}
