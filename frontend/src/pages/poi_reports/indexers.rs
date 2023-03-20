use std::collections::BTreeMap;

use gloo::timers::callback::Interval;
use graphql_client::{GraphQLQuery, Response as GraphQLResponse};
use log::warn;
use reqwasm::http::{Method, Request, RequestMode};
use stylist::css;
use wasm_bindgen_futures::spawn_local;
use yew::{html::Scope, prelude::*};
use yew_router::prelude::*;

use crate::contexts::{Theme, ThemeContext};
use crate::routes::Route;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "./graphql/api_schema.graphql",
    query_path = "./graphql/queries/poi_cross_check_reports.graphql",
    response_derives = "Debug, Clone"
)]
struct POICrossCheckReports;

type POICrossCheckReport = poi_cross_check_reports::PoiCrossCheckReportsPoiCrossCheckReports;

async fn fetch_indexer_cross_check_reports(
    indexer1: String,
    indexer2: String,
) -> Result<Vec<POICrossCheckReport>, anyhow::Error> {
    let query = POICrossCheckReports::build_query(poi_cross_check_reports::Variables {
        request: poi_cross_check_reports::POICrossCheckReportRequest {
            deployments: vec![],
            indexer1: Some(indexer1),
            indexer2: Some(indexer2),
        },
    });
    let query = serde_json::json!(query);

    let request = Request::new("http://localhost:3030/graphql")
        .body(query.to_string())
        .header("content-type", "application/json")
        .method(Method::POST)
        .mode(RequestMode::Cors);

    let response: GraphQLResponse<poi_cross_check_reports::ResponseData> =
        request.send().await?.json().await?;

    let reports = match (response.data, response.errors) {
        (Some(data), _) => data.poi_cross_check_reports,
        (_, Some(errors)) => {
            warn!("Errors fetching POI cross-check reports: {:?}", errors);
            vec![]
        }
        (_, _) => vec![],
    };

    Ok(reports)
}

fn poll_indexer_cross_check_reports(indexer1: String, indexer2: String, link: Scope<View>) {
    spawn_local(async move {
        let reports = match fetch_indexer_cross_check_reports(indexer1, indexer2).await {
            Ok(reports) => reports,
            Err(error) => {
                warn!("Failed to fetch POI cross-check reports: {}", error);
                return;
            }
        };

        let layout = Layout::from(reports);

        link.send_message(Msg::Update(layout));
    });
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub reports: Vec<POICrossCheckReport>,
}

impl Cell {
    fn render(&self, theme: &Theme) -> Html {
        match self.reports.is_empty() {
            true => html! { <td>{"-"}</td> },
            false => {
                let conflicts = self
                    .reports
                    .iter()
                    .filter(|report| report.proof_of_indexing1.ne(&report.proof_of_indexing2))
                    .collect::<Vec<_>>();

                match conflicts.is_empty() {
                    true => html! {
                        <td style={format!("background: {}", &theme.colors.ok)}>{"\u{00a0}"}</td>
                    },
                    false => {
                        let conflict = conflicts.iter().next().unwrap();

                        html! {
                            <td style={format!("background: {};", &theme.colors.error)}>
                                <Link<Route> to={Route::POIReportsForIndexers { indexer1: conflict.indexer1.clone(), indexer2: conflict.indexer2.clone() }}>
                                {
                                    format!(
                                        "{} {}",
                                        conflicts.len(),
                                        match conflicts.len() {
                                            1 => "conflict",
                                            _ => "conflicts"
                                        }
                                    )
                                }
                                </Link<Route>>
                            </td>
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub indexer: String,
    pub cells: Vec<Cell>,
}

impl Row {
    fn new(indexer: String, columns: usize) -> Self {
        let mut cells = vec![];
        cells.resize(columns, Default::default());
        Self { indexer, cells }
    }

    fn render(&self, theme: &Theme) -> Html {
        html! {
            <tr>
                <th>{&self.indexer}</th>
                {
                    for self.cells.iter().map(|cell| cell.render(theme))
                }
            </tr>
        }
    }
}

#[derive(Debug, Default)]
pub struct Layout {
    pub deployments: BTreeMap<String, Vec<POICrossCheckReport>>,
}

impl From<Vec<POICrossCheckReport>> for Layout {
    fn from(reports: Vec<POICrossCheckReport>) -> Self {
        let mut deployments = BTreeMap::<String, Vec<POICrossCheckReport>>::new();

        for report in reports {
            deployments
                .entry(report.deployment.clone())
                .or_default()
                .push(report);
        }

        Self { deployments }
    }
}

impl Layout {
    fn render(&self, theme: &Theme) -> Html {
        html! {
            <div class={css!(
                r#"
                "#
            )}>
            {
                match self.deployments.is_empty() {
                    true => html! { <p>{"No reports generated yet."}</p> },
                    false => html! {
                        <>
                        {
                            for self.deployments.iter().map(|(deployment, reports)| {
                                html! {
                                    <div class={css!(
                                        r#"
                                          table {
                                            /* table-layout: fixed; */
                                            width: 100%;
                                          }

                                          th { text-align: left; }

                                          th, td {
                                            padding: 0.5rem;
                                          }


                                          td {
                                            font-family: monospace;
                                          }
                                        "#
                                    )}>
                                        <h2>{"Deployment "}{&deployment}</h2>
                                        <table>
                                            <thead>
                                                <tr>
                                                  <th>{"Block"}</th>
                                                  <th>{"POI 1"}</th>
                                                  <th>{"POI 2"}</th>
                                                  <th>{"Diverging Block"}</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                <>
                                                {
                                                    for reports.iter().map(|report| {
                                                        html! {
                                                            <tr>
                                                              <td>{report.block.number}{" ("}{report.block.hash.clone().unwrap_or(String::from("-"))}{")"}</td>
                                                              <td>{&report.proof_of_indexing1[0..7]}</td>
                                                              <td>{&report.proof_of_indexing2[0..7]}</td>
                                                                <td>
                                                                  //<pre>{report.diverging_block.clone().map_or(String::from("-"), |diverging_block| format!("{:#?}", diverging_block))}</pre>
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                }
                                                </>
                                            </tbody>
                                        </table>
                                    </div>
                                }
                            })
                        }
                        </>
                    }
                }
            }
            </div>
        }
    }
}

pub struct View {
    layout: Layout,
    _poll_interval: Interval,
}

pub enum Msg {
    Update(Layout),
}

#[derive(PartialEq, Properties)]
pub struct ViewProperties {
    pub indexer1: String,
    pub indexer2: String,
}

impl Component for View {
    type Message = Msg;
    type Properties = ViewProperties;

    fn create(ctx: &Context<Self>) -> Self {
        let indexer1 = ctx.props().indexer1.clone();
        let indexer2 = ctx.props().indexer2.clone();

        let link = ctx.link().clone();

        poll_indexer_cross_check_reports(indexer1.clone(), indexer2.clone(), link.clone());

        Self {
            layout: Layout::default(),

            // Refetch reports every few seconds
            _poll_interval: Interval::new(5000, move || {
                poll_indexer_cross_check_reports(indexer1.clone(), indexer2.clone(), link.clone())
            }),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Update(layout) => {
                self.layout = layout;
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let (theme, _) = ctx
            .link()
            .context::<ThemeContext>(Callback::noop())
            .expect("theme context to be set");

        html! {
            <div>
                {self.layout.render(&theme)}
            </div>
        }
    }
}
