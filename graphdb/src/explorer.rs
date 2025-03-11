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
<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        h1, h3 {
            color: #333;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
            background: white;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1);
            border-radius: 4px;
        }
        th, td {
            padding: 12px 15px;
            text-align: left;
            border-bottom: 1px solid #ddd;
        }
        th {
            background-color: #f8f9fa;
            color: #666;
            font-weight: 600;
            width: 200px;
        }
        tr:nth-child(even) {
            background-color: #f8f9fa;
        }
        tr:hover {
            background-color: #f2f2f2;
        }
        a {
            color: #0366d6;
            text-decoration: none;
        }
        a:hover {
            text-decoration: underline;
        }
    </style>
</head>
<body>
    <h1>Atom {{ atom_id }}</h1>

    <h3>Properties</h3>
    <table>
        {% for prop in properties %}
        <tr>
            <th>{{ prop.name }}</th>
            <td>{{ prop.value }}</td>
        </tr>
        {% endfor %}
    </table>

    {% if outbound_triples %}
    <h3>Outbound Relations</h3>
    <table>
        <tr>
            <th>Triple ID</th>
            <th>Predicate</th>
            <th>Object</th>
        </tr>
        {% for triple in outbound_triples %}
        <tr>
            <td>{{ triple.id }}</td>
            <td><a href="/atom?id={{ triple.predicate }}">{{ triple.predicate_desc }}</a></td>
            <td><a href="/atom?id={{ triple.object }}">{{ triple.object_desc }}</a></td>
        </tr>
        {% endfor %}
    </table>
    {% endif %}

    {% if inbound_triples %}
    <h3>Inbound Relations</h3>
    <table>
        <tr>
            <th>Triple ID</th>
            <th>Subject</th>
            <th>Predicate</th>
        </tr>
        {% for triple in inbound_triples %}
        <tr>
            <td>{{ triple.id }}</td>
            <td><a href="/atom?id={{ triple.subject }}">{{ triple.subject_desc }}</a></td>
            <td><a href="/atom?id={{ triple.predicate }}">{{ triple.predicate_desc }}</a></td>
        </tr>
        {% endfor %}
    </table>
    {% endif %}
</body>
</html>
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
    let atom_id = App::atom_id_to_uuid(&query.id)?;
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
    context.insert("properties", &Vec::<serde_json::Value>::new());

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
        let properties = props
            .props
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name.to_string(),
                    "value": p.value.to_string()
                })
            })
            .collect::<Vec<_>>();
        context.insert("properties", &properties);
    }

    // Helper function to get atom description
    fn get_atom_description(db: &Database<MemoryDatastore>, id: &Uuid) -> String {
        let vertex_q = indradb::SpecificVertexQuery::new(vec![*id])
            .include()
            .properties()
            .unwrap();

        if let Ok(results) = db.get(vertex_q) {
            if let Some(props) = indradb::util::extract_vertex_properties(results)
                .unwrap()
                .first()
            {
                // Try to find description or label property
                for prop in &props.props {
                    if prop.name.to_string() == "description" || prop.name.to_string() == "label" {
                        // Remove quotes from the value
                        return prop
                            .value
                            .as_str()
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string();
                    }
                }
                // Fallback to type if no description found
                return props.vertex.t.to_string();
            }
        }
        id.simple().to_string() // Fallback to UUID if nothing found
    }

    // Extract and format outbound triples
    let outbound_triples = indradb::util::extract_edges(outbound_results)
        .unwrap()
        .into_iter()
        .map(|e| {
            // Get predicate description using the edge type as the atom ID
            let pred_desc = get_atom_description(&db, &e.outbound_id);

            // Get object description
            let obj_desc = get_atom_description(&db, &e.inbound_id);

            serde_json::json!({
                "id": e.outbound_id.simple().to_string(),  // Use outbound_id for ID
                "predicate": e.t.to_string(),  // Original atom ID for the link
                "predicate_desc": pred_desc,  // Description from the atom
                "object": e.inbound_id.simple().to_string(),
                "object_desc": obj_desc
            })
        })
        .collect::<Vec<_>>();

    // Extract and format inbound triples
    let inbound_triples = indradb::util::extract_edges(inbound_results)
        .unwrap()
        .into_iter()
        .map(|e| {
            // Get subject description
            let subj_desc = get_atom_description(&db, &e.outbound_id);

            // Get predicate description using the edge type as the atom ID
            let pred_desc = get_atom_description(&db, &e.inbound_id);

            serde_json::json!({
                "id": e.inbound_id.simple().to_string(),  // Use inbound_id for ID
                "subject": e.outbound_id.simple().to_string(),
                "subject_desc": subj_desc,
                "predicate": e.t.to_string(),  // Original atom ID for the link
                "predicate_desc": pred_desc  // Description from the atom
            })
        })
        .collect::<Vec<_>>();

    context.insert("outbound_triples", &outbound_triples);
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
