use std::collections::{BTreeMap, BTreeSet};

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

async fn fetch_cross_check_reports() -> Result<Vec<POICrossCheckReport>, anyhow::Error> {
    let query = POICrossCheckReports::build_query(poi_cross_check_reports::Variables {
        request: poi_cross_check_reports::POICrossCheckReportRequest {
            deployments: vec![],
            indexer1: None,
            indexer2: None,
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

fn poll_cross_check_reports(link: Scope<View>) {
    spawn_local(async move {
        let reports = match fetch_cross_check_reports().await {
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
    pub indexer_indices: BTreeMap<String, usize>,
    pub rows: Vec<Row>,
}

impl From<Vec<POICrossCheckReport>> for Layout {
    fn from(reports: Vec<POICrossCheckReport>) -> Self {
        let indexer_indices = reports
            .iter()
            .map(|report| report.indexer1.clone())
            .chain(reports.iter().map(|report| report.indexer2.clone()))
            .collect::<BTreeSet<String>>()
            .into_iter()
            .enumerate()
            .map(|(i, indexer)| (indexer, i))
            .collect::<BTreeMap<String, usize>>();

        let mut rows = vec![];
        for indexer in indexer_indices.keys() {
            rows.push(Row::new(indexer.clone(), indexer_indices.len()));
        }

        for report in reports {
            let indexer1_index = indexer_indices.get(&report.indexer1).unwrap();
            let indexer2_index = indexer_indices.get(&report.indexer2).unwrap();

            rows[*indexer1_index].cells[*indexer2_index]
                .reports
                .push(report.clone());
            rows[*indexer2_index].cells[*indexer1_index]
                .reports
                .push(report);
        }

        Self {
            indexer_indices,
            rows,
        }
    }
}

impl Layout {
    fn render(&self, theme: &Theme) -> Html {
        html! {
            <div class={css!(
                r#"
                  table {
                    table-layout: fixed;
                    width: 28rem;
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
                <table cellspacing="0">
                    <thead>
                      <th> </th>
                      {
                          for self.indexer_indices.keys().map(|indexer| {
                              html! {
                                  <th>{&indexer}</th>
                              }
                          })
                      }
                    </thead>
                    <tbody>
                      {
                          for self.rows.iter().map(|row| row.render(theme))
                      }
                    </tbody>
                </table>
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

impl Component for View {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();

        poll_cross_check_reports(link.clone());

        Self {
            layout: Layout::default(),

            // Refetch reports every few seconds
            _poll_interval: Interval::new(5000, move || poll_cross_check_reports(link.clone())),
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
