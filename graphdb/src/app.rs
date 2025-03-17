use std::str::FromStr;

use crate::error::GraphDBError;
use crate::graphql;
use graphql_client::{GraphQLQuery, Response};
use indradb::{Database, Edge, Identifier, Json, MemoryDatastore, SpecificVertexQuery, Vertex};
use md5;
use models::{
    atom::Atom,
    traits::{Paginated, SimpleCrud},
    triple::Triple,
    types::U256Wrapper,
};
use reqwest;
use serde::{Deserialize, Serialize};
use shared_utils::postgres::ceiling_div;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GraphTriple {
    pub id: i64,
    pub subject_id: i64,
    pub subject: GraphAtom,
    pub predicate_id: i64,
    pub predicate: GraphAtom,
    pub object_id: i64,
    pub object: GraphAtom,
    pub creator_id: String,
}

#[derive(Debug, Clone)]
pub struct GraphAtom {
    pub id: i64,
    pub label: String,
    pub atom_type: String,
}

impl GraphAtom {
    pub fn properties(&self) -> Vec<(String, Json)> {
        vec![
            ("id".into(), Json::new(self.id.into())),
            ("label".into(), Json::new(self.label.clone().into())),
            ("atom_type".into(), Json::new(self.atom_type.clone().into())),
        ]
    }
}

#[derive(Deserialize)]
pub struct Env {
    pub database_url: String,
}
pub struct App {
    pub env: Env,
    pub db: Database<MemoryDatastore>,
    // pub pg_pool: PgPool,
}

impl App {
    /// The page size for the triples.
    pub const PAGE_SIZE: i64 = 10;

    /// Initialize the environment variables.
    pub async fn initialize() -> Result<Env, GraphDBError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Load the environment variables into our struct
        let env = envy::from_env::<Env>().map_err(GraphDBError::from)?;
        Ok(env)
    }

    pub async fn new() -> Result<Self, GraphDBError> {
        let env = Self::initialize().await?;
        // let pg_pool = Self::connect_to_db(&env.database_url).await?;
        let db = Self::create_db();
        Ok(Self { db, env })
    }
    pub fn create_db() -> Database<MemoryDatastore> {
        let db: Database<MemoryDatastore> = MemoryDatastore::new_db();
        db
    }

    /// This function connects to the postgres database.
    pub async fn connect_to_db(database_url: &str) -> Result<PgPool, GraphDBError> {
        PgPoolOptions::new()
            .min_connections(5)
            .max_connections(20)
            .connect(database_url)
            .await
            .map_err(|error| GraphDBError::PostgresConnectError(error.to_string()))
    }

    /// This function sets the properties for an atom.
    pub async fn set_properties_for_atom(
        &self,
        atom: &GraphAtom,
        vertex_id: Uuid,
    ) -> Result<(), GraphDBError> {
        for (key, value) in atom.properties() {
            self.db.set_properties(
                SpecificVertexQuery::new(vec![vertex_id]),
                Identifier::new(key)?,
                &value,
            )?;
        }
        Ok(())
    }

    pub async fn get_triples_count(&self) -> Result<i64, GraphDBError> {
        let client = reqwest::Client::new();

        // Manual GraphQL query without using the client's deserialization
        let query_json = serde_json::json!({
            "query": "query GetTriplesCount { triples_aggregate { aggregate { count } } }"
        });

        let res = client
            .post("https://prod.base-sepolia.intuition-api.com/v1/graphql")
            .json(&query_json)
            .send()
            .await
            .map_err(|e| GraphDBError::TripleCountError(e.to_string()))?;

        // Parse directly with serde_json
        let text = res
            .text()
            .await
            .map_err(|e| GraphDBError::TripleCountError(e.to_string()))?;

        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| GraphDBError::TripleCountError(format!("JSON parse error: {}", e)))?;

        // Extract count using path access
        let count = parsed["data"]["triples_aggregate"]["aggregate"]["count"]
            .as_i64()
            .unwrap_or(0);

        Ok(count)
    }

    pub async fn upload_triples(&self) -> Result<(), GraphDBError> {
        let total_count_triples = self.get_triples_count().await?;
        let total_pages = ceiling_div(total_count_triples, Self::PAGE_SIZE).await;
        for page in 1..total_pages {
            let triples = self
                .get_triples(Self::PAGE_SIZE, (page - 1) * Self::PAGE_SIZE)
                .await?;

            // For each triple we need to create an edge in the database, but we also
            // need to create the vertices if they don't exist
            for triple in triples {
                // Use the atom data already included in the triple
                let subject_atom = triple.subject;
                let predicate_atom = triple.predicate;
                let object_atom = triple.object;

                // Create vertices with type
                let subject_vertex = Vertex::with_id(
                    Self::atom_id_to_uuid(&subject_atom.id.to_string())?,
                    Identifier::new(subject_atom.atom_type.to_string())?,
                );
                let predicate_vertex = Vertex::with_id(
                    Self::atom_id_to_uuid(&predicate_atom.id.to_string())?,
                    Identifier::new(predicate_atom.atom_type.to_string())?,
                );
                let object_vertex = Vertex::with_id(
                    Self::atom_id_to_uuid(&object_atom.id.to_string())?,
                    Identifier::new(object_atom.atom_type.to_string())?,
                );

                // Create vertices
                self.db.create_vertex(&subject_vertex)?;
                self.db.create_vertex(&predicate_vertex)?;
                self.db.create_vertex(&object_vertex)?;

                // Set vertex properties
                self.set_properties_for_atom(&subject_atom, subject_vertex.id)
                    .await?;
                self.set_properties_for_atom(&predicate_atom, predicate_vertex.id)
                    .await?;
                self.set_properties_for_atom(&object_atom, object_vertex.id)
                    .await?;

                // Create edge with type
                let link_type = Identifier::new(predicate_atom.id.to_string())?;
                let edge = Edge::new(subject_vertex.id, link_type, object_vertex.id);
                self.db.create_edge(&edge)?;
            }
        }
        Ok(())
    }

    pub async fn get_triples(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<GraphTriple>, GraphDBError> {
        let client = reqwest::Client::new();

        // Manual GraphQL query with variables
        let query_json = serde_json::json!({
            "query": "query GetTriples($limit: Int!, $offset: Int!) {
                triples(limit: $limit, offset: $offset) { 
                    id subject_id subject { id label type } 
                    predicate_id predicate { id label type } 
                    object_id object { id label type } 
                    creator_id 
                } 
            }",
            "variables": {
                "limit": limit,
                "offset": offset
            }
        });

        println!("Sending triples query: {:?}", query_json);

        // Send request
        let response = client
            .post("https://prod.base-sepolia.intuition-api.com/v1/graphql")
            .header("Content-Type", "application/json")
            .json(&query_json)
            .send()
            .await
            .map_err(|e| GraphDBError::GetTripleError(format!("Request error: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(GraphDBError::GetTripleError(format!(
                "HTTP error: {} - {}",
                status, error_text
            )));
        }

        // Get text
        let text = response
            .text()
            .await
            .map_err(|e| GraphDBError::GetTripleError(format!("Text error: {}", e)))?;

        println!("Response body: {}", text);

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| GraphDBError::GetTripleError(format!("JSON parse error: {}", e)))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors") {
            return Err(GraphDBError::GetTripleError(format!(
                "GraphQL errors: {:?}",
                errors
            )));
        }

        // Extract triples
        let triples_json = json
            .get("data")
            .and_then(|d| d.get("triples"))
            .ok_or_else(|| {
                GraphDBError::GetTripleError("No triples data in response".to_string())
            })?;

        // Convert to Vec<GraphTriple>
        let mut triples = Vec::new();

        if let Some(triples_array) = triples_json.as_array() {
            for t in triples_array {
                let triple = GraphTriple {
                    id: t["id"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0),
                    subject_id: t["subject_id"]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<i64>()
                        .unwrap_or(0),
                    subject: GraphAtom {
                        id: t["subject"]["id"]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<i64>()
                            .unwrap_or(0),
                        label: t["subject"]["label"].as_str().unwrap_or("").to_string(),
                        atom_type: t["subject"]["type"].as_str().unwrap_or("").to_string(),
                    },
                    predicate_id: t["predicate_id"]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<i64>()
                        .unwrap_or(0),
                    predicate: GraphAtom {
                        id: t["predicate"]["id"]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<i64>()
                            .unwrap_or(0),
                        label: t["predicate"]["label"].as_str().unwrap_or("").to_string(),
                        atom_type: t["predicate"]["type"].as_str().unwrap_or("").to_string(),
                    },
                    object_id: t["object_id"]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<i64>()
                        .unwrap_or(0),
                    object: GraphAtom {
                        id: t["object"]["id"]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<i64>()
                            .unwrap_or(0),
                        label: t["object"]["label"].as_str().unwrap_or("").to_string(),
                        atom_type: t["object"]["type"].as_str().unwrap_or("").to_string(),
                    },
                    creator_id: t["creator_id"].as_str().unwrap_or("").to_string(),
                };
                triples.push(triple);
            }
        }

        Ok(triples)
    }

    pub async fn get_atoms(&self, limit: i64, offset: i64) -> Result<Vec<GraphAtom>, GraphDBError> {
        let client = reqwest::Client::new();

        // Manual GraphQL query with variables
        let query_json = serde_json::json!({
            "query": "query GetAtoms($limit: Int!, $offset: Int!) { atoms(limit: $limit, offset: $offset) { id label type creator_id } }",
            "variables": {
                "limit": limit,
                "offset": offset
            }
        });

        // Send request
        let response = client
            .post("https://prod.base-sepolia.intuition-api.com/v1/graphql")
            .header("Content-Type", "application/json")
            .json(&query_json)
            .send()
            .await
            .map_err(|e| GraphDBError::PostgresConnectError(format!("Request error: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(GraphDBError::PostgresConnectError(format!(
                "HTTP error: {} - {}",
                status, error_text
            )));
        }

        // Get text
        let text = response
            .text()
            .await
            .map_err(|e| GraphDBError::PostgresConnectError(format!("Text error: {}", e)))?;

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| GraphDBError::PostgresConnectError(format!("JSON parse error: {}", e)))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors") {
            return Err(GraphDBError::PostgresConnectError(format!(
                "GraphQL errors: {:?}",
                errors
            )));
        }

        // Extract atoms
        let atoms_json = json
            .get("data")
            .and_then(|d| d.get("atoms"))
            .ok_or_else(|| {
                GraphDBError::PostgresConnectError("No atoms data in response".to_string())
            })?;

        // Convert to Vec<GraphAtom>
        let mut atoms = Vec::new();

        if let Some(atoms_array) = atoms_json.as_array() {
            for a in atoms_array {
                let atom = GraphAtom {
                    id: a["id"].as_i64().unwrap_or(0),
                    label: a["label"].as_str().unwrap_or("").to_string(),
                    atom_type: a["type"].as_str().unwrap_or("").to_string(),
                };
                atoms.push(atom);
            }
        }

        Ok(atoms)
    }

    pub async fn get_atom_by_id(&self, id: i64) -> Result<Option<GraphAtom>, GraphDBError> {
        let client = reqwest::Client::new();

        // Manual GraphQL query with variables
        let query_json = serde_json::json!({
            "query": "query GetAtomById($id: numeric!) { atom(id: $id) { id label type creator_id } }",
            "variables": {
                "id": id
            }
        });

        // Send request
        let response = client
            .post("https://prod.base-sepolia.intuition-api.com/v1/graphql")
            .header("Content-Type", "application/json")
            .json(&query_json)
            .send()
            .await
            .map_err(|e| GraphDBError::PostgresConnectError(format!("Request error: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(GraphDBError::PostgresConnectError(format!(
                "HTTP error: {} - {}",
                status, error_text
            )));
        }

        // Get text
        let text = response
            .text()
            .await
            .map_err(|e| GraphDBError::PostgresConnectError(format!("Text error: {}", e)))?;

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| GraphDBError::PostgresConnectError(format!("JSON parse error: {}", e)))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors") {
            return Err(GraphDBError::PostgresConnectError(format!(
                "GraphQL errors: {:?}",
                errors
            )));
        }

        // Extract atom
        let atom_json = json.get("data").and_then(|d| d.get("atom"));

        // If atom is null, return None
        if atom_json.is_none() || atom_json.unwrap().is_null() {
            return Ok(None);
        }

        // Convert to GraphAtom
        let atom_json = atom_json.unwrap();
        let atom = GraphAtom {
            id: atom_json["id"].as_i64().unwrap_or(0),
            label: atom_json["label"].as_str().unwrap_or("").to_string(),
            atom_type: atom_json["type"].as_str().unwrap_or("").to_string(),
        };

        Ok(Some(atom))
    }

    pub fn atom_id_to_uuid(atom_id: &str) -> Result<Uuid, GraphDBError> {
        println!("Converting atom ID to UUID: {}", atom_id);
        let namespace = Uuid::NAMESPACE_URL;
        let id_str = format!("atom:{}", atom_id);
        let uuid = Uuid::new_v5(&namespace, id_str.as_bytes());
        println!("Generated UUID: {}", uuid);
        Ok(uuid)
    }
}
