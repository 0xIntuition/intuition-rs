use indradb::{Database, Identifier, MemoryDatastore, QueryExt};
use serde::Deserialize;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tera::{Context as TeraContext, Tera};
use uuid::Uuid;
use warp::{Filter, http, reject, reply};

use crate::app::App;
use crate::error::GraphDBError;

const INDEX: &str = r#"
<form method="get" action="/atom">
    <input name="id" value="" type="text" placeholder="Enter atom ID"/>
    <button type="submit">Get Atom</button>
</form>
"#;

const ATOM_TEMPLATE: &str = r#"
<h1>Atom {{ atom_id }}</h1>

<h3>Properties</h3>
<table>
    <tr>
        <th>Type</th>
        <td>{{ atom_type }}</td>
    </tr>
    <tr>
        <th>Label</th>
        <td>{{ label }}</td>
    </tr>
</table>

{% if outbound_triples %}
<h3>Outbound Relations</h3>
<table>
    <tr>
        <th>Predicate</th>
        <th>Object</th>
    </tr>
    {% for triple in outbound_triples %}
    <tr>
        <td><a href="/atom?id={{ triple.predicate }}">{{ triple.predicate }}</a></td>
        <td><a href="/atom?id={{ triple.object }}">{{ triple.object }}</a></td>
    </tr>
    {% endfor %}
</table>
{% endif %}

{% if inbound_triples %}
<h3>Inbound Relations</h3>
<table>
    <tr>
        <th>Subject</th>
        <th>Predicate</th>
    </tr>
    {% for triple in inbound_triples %}
    <tr>
        <td><a href="/atom?id={{ triple.subject }}">{{ triple.subject }}</a></td>
        <td><a href="/atom?id={{ triple.predicate }}">{{ triple.predicate }}</a></td>
    </tr>
    {% endfor %}
</table>
{% endif %}
"#;

#[derive(Deserialize)]
struct AtomQuery {
    id: String,
}

async fn handle_index() -> Result<impl warp::Reply, Infallible> {
    Ok(reply::html(INDEX))
}

async fn handle_atom(
    db: Arc<Database<MemoryDatastore>>,
    tera: Tera,
    query: AtomQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("Searching for atom ID: {}", query.id); // Debug
    let atom_id = App::atom_id_to_uuid(&query.id).map_err(|e| reject::custom(e))?;
    println!("Using UUID: {}", atom_id); // Debug

    // Get the vertex and its properties
    let vertex_q = indradb::SpecificVertexQuery::new(vec![atom_id])
        .include()
        .properties()
        .unwrap();

    let results = db
        .get(vertex_q)
        .map_err(|e| reject::custom(GraphDBError::IndradbError(e)))?;

    // Get outbound triples
    let outbound_q = indradb::SpecificVertexQuery::new(vec![atom_id])
        .outbound()
        .unwrap();
    let outbound_results = db
        .get(outbound_q)
        .map_err(|e| reject::custom(GraphDBError::IndradbError(e)))?;

    // Get inbound triples
    let inbound_q = indradb::SpecificVertexQuery::new(vec![atom_id])
        .inbound()
        .unwrap();
    let inbound_results = db
        .get(inbound_q)
        .map_err(|e| reject::custom(GraphDBError::IndradbError(e)))?;

    let mut context = TeraContext::new();
    context.insert("atom_id", &query.id);
    context.insert("atom_type", "Not found");
    context.insert("label", "Not found");

    // Extract vertex properties
    if let Some(props) = indradb::util::extract_vertex_properties(results)
        .unwrap()
        .first()
    {
        context.insert("atom_type", &props.vertex.t.to_string());
        for prop in &props.props {
            if prop.name.to_string() == "label" {
                context.insert("label", &prop.value);
            }
        }
    }

    // Extract and format outbound triples
    let outbound_triples = indradb::util::extract_edges(outbound_results)
        .unwrap()
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "predicate": e.t.to_string(),
                "object": e.inbound_id.simple().to_string()
            })
        })
        .collect::<Vec<_>>();
    context.insert("outbound_triples", &outbound_triples);

    // Extract and format inbound triples
    let inbound_triples = indradb::util::extract_edges(inbound_results)
        .unwrap()
        .into_iter()
        .map(|e| {
            // Need to reverse-lookup the original atom ID here
            let vertex_q = indradb::SpecificVertexQuery::new(vec![e.outbound_id])
                .include()
                .properties()
                .unwrap();
            let vertex = db.get(vertex_q).unwrap();
            let props = indradb::util::extract_vertex_properties(vertex).unwrap();
            let atom_id = props
                .first()
                .map(|p| p.vertex.t.to_string())
                .unwrap_or_default();

            serde_json::json!({
                "subject": atom_id,
                "predicate": e.t.to_string()
            })
        })
        .collect::<Vec<_>>();
    context.insert("inbound_triples", &inbound_triples);

    let rendered = tera
        .render("atom.html", &context)
        .map_err(|e| reject::custom(GraphDBError::Tera(e)))?;
    Ok(reply::html(rendered))
}

fn with_db(
    db: Arc<Database<MemoryDatastore>>,
) -> impl Filter<Extract = (Arc<Database<MemoryDatastore>>,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn with_tera(tera: Tera) -> impl Filter<Extract = (Tera,), Error = Infallible> + Clone {
    warp::any().map(move || tera.clone())
}

pub async fn run(db: Database<MemoryDatastore>, port: u16) -> Result<(), GraphDBError> {
    let db = Arc::new(db);
    let mut tera = Tera::default();
    tera.add_raw_templates(vec![("atom.html", ATOM_TEMPLATE)])?;

    let index_route = warp::path::end().and(warp::get()).and_then(handle_index);

    let atom_route = warp::path("atom")
        .and(warp::get())
        .and(with_db(db))
        .and(with_tera(tera))
        .and(warp::query::<AtomQuery>())
        .and_then(handle_atom);

    let routes = index_route.or(atom_route);

    println!("Starting explorer on http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;

    Ok(())
}
