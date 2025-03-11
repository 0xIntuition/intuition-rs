use std::str::FromStr;

use crate::error::GraphDBError;
use indradb::{Database, Edge, Identifier, MemoryDatastore, Vertex};
use models::{
    atom::Atom,
    traits::{Paginated, SimpleCrud},
    triple::Triple,
};
use serde::Deserialize;
use shared_utils::postgres::ceiling_div;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Env {
    pub database_url: String,
}
pub struct App {
    pub env: Env,
    pub db: Database<MemoryDatastore>,
    pub pg_pool: PgPool,
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
        let pg_pool = Self::connect_to_db(&env.database_url).await?;
        let db = Self::create_db();
        Ok(Self { db, pg_pool, env })
    }
    pub fn create_db() -> Database<MemoryDatastore> {
        let db: Database<MemoryDatastore> = MemoryDatastore::new_db();
        db
    }

    pub async fn connect_to_db(database_url: &str) -> Result<PgPool, GraphDBError> {
        PgPoolOptions::new()
            .min_connections(5)
            .max_connections(20)
            .connect(database_url)
            .await
            .map_err(|error| GraphDBError::PostgresConnectError(error.to_string()))
    }

    pub async fn upload_triples(&self) -> Result<(), GraphDBError> {
        let total_count_triples = Triple::get_total_count(&self.pg_pool, "public").await?;
        let total_pages = ceiling_div(total_count_triples, Self::PAGE_SIZE).await;
        for page in 1..total_pages {
            let triples =
                Triple::get_paginated(&self.pg_pool, page, Self::PAGE_SIZE, "public").await?;
            // For each triple we need to create an edge in the database, but we also
            // need to create the vertices if they dont exist. Triples contain the ids of
            // the vertices, so we can use them to create the vertices, but we need to fetch
            // the complementary information from the postgres database.
            for triple in triples {
                let subject = triple.subject_id;
                let predicate = triple.predicate_id;
                let object = triple.object_id;

                let subject_atom = Atom::find_by_id(subject, &self.pg_pool, "public")
                    .await?
                    .ok_or(GraphDBError::AtomNotFound)?;
                let predicate_atom = Atom::find_by_id(predicate, &self.pg_pool, "public")
                    .await?
                    .ok_or(GraphDBError::AtomNotFound)?;
                let object_atom = Atom::find_by_id(object, &self.pg_pool, "public")
                    .await?
                    .ok_or(GraphDBError::AtomNotFound)?;

                // Create vertices with type
                let subject_vertex = Vertex::with_id(
                    Uuid::from_str(&subject_atom.id.to_string())?,
                    Identifier::new(subject_atom.atom_type.to_string())?,
                );
                let predicate_vertex = Vertex::with_id(
                    Uuid::from_str(&predicate_atom.id.to_string())?,
                    Identifier::new(predicate_atom.atom_type.to_string())?,
                );
                let object_vertex = Vertex::with_id(
                    Uuid::from_str(&object_atom.id.to_string())?,
                    Identifier::new(object_atom.atom_type.to_string())?,
                );

                // Create vertices
                self.db.create_vertex(&subject_vertex)?;
                self.db.create_vertex(&predicate_vertex)?;
                self.db.create_vertex(&object_vertex)?;

                // Create edge with type
                let link_type = Identifier::new("triple")?;
                let edge = Edge::new(subject_vertex.id, link_type, object_vertex.id);
                self.db.create_edge(&edge)?;
            }
        }
        Ok(())
    }
}
