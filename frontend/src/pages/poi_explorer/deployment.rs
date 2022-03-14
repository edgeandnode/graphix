use std::collections::BTreeMap;

use css_colors::{rgba, Color, Ratio, RGBA};
use gloo::timers::callback::Interval;
use graphql_client::{GraphQLQuery, Response as GraphQLResponse};
use log::warn;
use reqwasm::http::*;
use stylist::css;
use wasm_bindgen_futures::spawn_local;
use yew::{html::Scope, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "./graphql/api_schema.graphql",
    query_path = "./graphql/queries/proofs_of_indexing.graphql",
    response_derives = "Debug"
)]
struct ProofsOfIndexing;

type POI = proofs_of_indexing::ProofsOfIndexingProofsOfIndexing;
type POISGroupedByBlock = BTreeMap<i64, Vec<POI>>;

async fn fetch_proofs_of_indexing(deployment: String) -> Result<POISGroupedByBlock, anyhow::Error> {
    let query = ProofsOfIndexing::build_query(proofs_of_indexing::Variables {
        request: proofs_of_indexing::ProofOfIndexingRequest {
            deployments: vec![deployment.clone()],
            blockRange: None,
            limit: None,
        },
    });
    let query = serde_json::json!(query);

    let request = Request::new("http://localhost:3030/graphql")
        .body(query.to_string())
        .header("content-type", "application/json")
        .method(Method::POST)
        .mode(RequestMode::Cors);

    let response: GraphQLResponse<proofs_of_indexing::ResponseData> =
        request.send().await?.json().await?;

    let pois = match (response.data, response.errors) {
        (Some(data), _) => data.proofs_of_indexing,
        (_, Some(errors)) => {
            warn!(
                "Errors fetching proofs of indexing for deployment {}: {:?}",
                deployment, errors
            );
            vec![]
        }
        (_, _) => vec![],
    };

    // Group POIs by block number (using a BTreeMap to order the numbers)
    let mut grouped_by_block = POISGroupedByBlock::new();

    for poi in pois {
        grouped_by_block
            .entry(poi.block.number)
            .or_insert_with(Vec::new)
            .push(poi);
    }

    // For each block number, sort POIs by indexer
    for (_, pois) in grouped_by_block.iter_mut() {
        pois.sort_by(|a, b| a.indexer.cmp(&b.indexer));
    }

    Ok(grouped_by_block)
}

fn poll_proofs_of_indexing(deployment: String, link: Scope<View>) {
    spawn_local(async move {
        let pois = match fetch_proofs_of_indexing(deployment).await {
            Ok(pois) => pois,
            Err(error) => {
                warn!("Failed to fetch proofs of indexing: {}", error);
                return;
            }
        };

        let layout = Layout::from(pois);

        link.send_message(Msg::Update(layout));
    });
}

#[derive(Debug)]
pub enum Cell {
    POI(POI, RGBA),
    Placeholder,
}

impl Cell {
    fn render(&self) -> Html {
        match self {
            Cell::POI(poi, color) => html! {
                <td style={format!("background-color: {};", color.to_css())}>
                  {&poi.proof_of_indexing[..7]}
                </td>
            },
            Cell::Placeholder => html! {
                <td>{"-"}</td>
            },
        }
    }
}

#[derive(Debug)]
pub enum Row {
    Block(i64, Vec<Cell>),
    Placeholder,
}

impl Row {
    fn render(&self) -> Html {
        match self {
            Row::Block(number, cells) => html! {
                <tr>
                    <td>{number}</td>
                    <>
                        {
                            for cells.iter().map(Cell::render)
                        }
                    </>
                </tr>
            },
            Row::Placeholder => html! {
                <tr>
                    <td class="placeholder">{"..."}</td>
                </tr>
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct Layout {
    pub indexers: Vec<String>,
    pub rows: Vec<Row>,
}

impl Layout {
    fn from(data: POISGroupedByBlock) -> Self {
        let mut rows = vec![];

        // Assign a column to each indexer
        let mut indexer_columns = BTreeMap::new();
        let mut indexers = vec![];
        for group in data.values() {
            for poi in group {
                match indexer_columns.get(&poi.indexer) {
                    Some(_) => {}
                    None => {
                        indexer_columns.insert(poi.indexer.clone(), indexers.len());
                        indexers.push(poi.indexer.clone());
                    }
                }
            }
        }

        let mut groups = data.into_iter().rev().peekable();
        while let Some((block_number, group)) = groups.next() {
            // Create a new row for the block number
            let mut cells = vec![];
            let mut color = rgba(17, 157, 164, 1.0).lighten(Ratio::from_percentage(40));
            let mut last_poi = None;

            // Add indexer POIs or placeholders
            for poi in group {
                // Identify the indexer column
                let indexer_column = indexer_columns.get(&poi.indexer).unwrap();

                // If the indexer is supposed to be at a later column, insert placeholders
                // for every column (indexer) for which we don't have data
                let current_column = cells.len();
                if *indexer_column > current_column {
                    for _ in current_column..*indexer_column {
                        cells.push(Cell::Placeholder);
                    }
                }

                if last_poi.is_none() || poi.proof_of_indexing.ne(last_poi.as_ref().unwrap()) {
                    last_poi = Some(poi.proof_of_indexing.clone());
                    color = color.darken(Ratio::from_percentage(15));
                }

                cells.push(Cell::POI(poi, color));
            }

            // Add the row to the layout
            rows.push(Row::Block(block_number, cells));

            // If the next block number is not one block before, add a placeholder
            if let Some((next_block_number, _)) = groups.peek() {
                if next_block_number < &(block_number - 1) {
                    rows.push(Row::Placeholder);
                }
            }
        }

        Self { indexers, rows }
    }

    fn render(&self) -> Html {
        html! {
            <div class={css!(
                r#"
                  table {
                    table-layout: fixed;
                    width: 20rem;
                  }

                  th { text-align: left; }

                  th, td {
                    padding: 0.5rem;
                  }


                  td {
                    font-family: monospace;
                  }

                  .placeholder {
                    writing-mode: vertical-lr;
                    text-align: center;
                    text-orientation: upright;
                    font-size: xx-small;
                    padding: 0;
                  }
                "#
            )}>
                <table cellspacing="0">
                    <thead>
                        <tr>
                            <th>{"Block"}</th>
                                {
                                    for self.indexers.iter().map(|i| html! {
                                        <th>{i}</th>
                                    })
                                }
                            <>
                            </>
                        </tr>
                    </thead>
                    <tbody>
                        {
                            for self.rows.iter().map(Row::render)
                        }
                    </tbody>
                </table>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ViewProps {
    pub id: String,
}

pub struct View {
    layout: Layout,
    _proofs_of_indexing_interval: Interval,
}

pub enum Msg {
    Update(Layout),
}

impl Component for View {
    type Message = Msg;
    type Properties = ViewProps;

    fn create(ctx: &Context<Self>) -> Self {
        let id = ctx.props().id.clone();
        let link = ctx.link().clone();

        // Fetch deployments immediately
        poll_proofs_of_indexing(id.clone(), link.clone());

        Self {
            layout: Layout::default(),

            // Refetch deployments every 5s
            _proofs_of_indexing_interval: Interval::new(5000, move || {
                poll_proofs_of_indexing(id.clone(), link.clone())
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

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class={css!(
               r#"
               "#
            )}>
                {self.layout.render()}
            </div>
        }
    }
}
