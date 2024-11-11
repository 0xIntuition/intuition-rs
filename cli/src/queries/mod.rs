use graphql_client::GraphQLQuery;
type Numeric = i64;
type Float8 = f64;
type AtomType = String;
type AccountType = String;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/queries/schema.graphql",
    query_path = "src/queries/get-accounts.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct GetAccounts;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/queries/schema.graphql",
    query_path = "src/queries/get-account-info.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct GetAccountInfo;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/queries/schema.graphql",
    query_path = "src/queries/get-atoms.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct GetAtoms;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/queries/schema.graphql",
    query_path = "src/queries/get-signals.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct GetSignals;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/queries/schema.graphql",
    query_path = "src/queries/get-predicate-objects.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct GetPredicateObjects;
