use std::str::FromStr;

use crate::error::GraphDBError;
use indradb::{Database, Edge, Identifier, Json, MemoryDatastore, SpecificVertexQuery, Vertex};
use md5;
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
        atom: &Atom,
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

                // Creates a new vertex. Returns whether the vertex was successfully
                // created - if this is false, it's because a vertex with the same UUID
                // already exists.
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
                let link_type = Identifier::new(predicate_atom.atom_type.to_string())?;
                let edge = Edge::new(subject_vertex.id, link_type, object_vertex.id);
                self.db.create_edge(&edge)?;
            }
        }
        Ok(())
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
