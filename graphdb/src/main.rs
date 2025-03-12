use app::App;
use error::GraphDBError;

pub mod app;
pub mod error;
pub mod explorer;
// use indradb;

// pub fn create_db() -> Result<(), indradb::Error> {
//     // Create an in-memory datastore
//     let db: indradb::Database<indradb::MemoryDatastore> = indradb::MemoryDatastore::new_db();

//     // Create a couple of vertices
//     let out_v = indradb::Vertex::new(indradb::Identifier::new("person")?);
//     let in_v = indradb::Vertex::new(indradb::Identifier::new("movie")?);
//     db.create_vertex(&out_v)?;
//     db.create_vertex(&in_v)?;

//     // Add an edge between the vertices
//     let edge = indradb::Edge::new(out_v.id, indradb::Identifier::new("likes")?, in_v.id);
//     db.create_edge(&edge)?;

//     // Query for the edge
//     let output: Vec<indradb::QueryOutputValue> =
//         db.get(indradb::SpecificEdgeQuery::single(edge.clone()))?;
//     // Convenience function to extract out the edges from the query results
//     let e = indradb::util::extract_edges(output).unwrap();
//     assert_eq!(e.len(), 1);
//     assert_eq!(edge, e[0]);
//     println!("Database created successfully");
//     Ok(())
// }

#[tokio::main]
async fn main() -> Result<(), GraphDBError> {
    let app = App::new().await?;
    app.upload_triples().await?;

    // Start the web interface
    explorer::run(app.db, 3000).await?;
    Ok(())
}
