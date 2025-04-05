use graphql_client::GraphQLQuery;

// Match these exactly with what's used in app.rs
pub type AtomType = String;
pub type Numeric = i64;
pub type atom_type = String;
pub type numeric = i64;

// Other types needed by your schema
pub type Bigint = String;
pub type Float8 = f64;
pub type AccountType = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.graphql",
    query_path = "src/graphql/queries/get_triples.graphql",
    response_derives = "Debug"
)]
pub struct GetTriples;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.graphql",
    query_path = "src/graphql/queries/get_atoms.graphql",
    response_derives = "Debug"
)]
pub struct GetAtoms;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.graphql",
    query_path = "src/graphql/queries/get_atoms.graphql",
    response_derives = "Debug"
)]
pub struct GetAtomById;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.graphql",
    query_path = "src/graphql/queries/get_triples_count.graphql",
    response_derives = "Debug"
)]
pub struct GetTriplesCount;
