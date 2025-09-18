use crate::queries::{
    get_atoms::{GetAtoms, Variables as GetAtomsVariables},
    get_atoms_detailed::{GetAtomsDetailed, Variables as GetAtomsDetailedVariables},
    get_atoms_simple::{GetAtomsSimple, Variables as GetAtomsSimpleVariables},
};
use graphql_client::GraphQLQuery;
use reqwest::Client;

const GRAPHQL_ENDPOINT: &str = "http://localhost:8080/v1/graphql";

/// Example: Fetch all atoms with basic fields
pub async fn fetch_all_atoms_basic()
-> Result<Vec<get_atoms_simple::GetAtomsSimpleAtoms>, Box<dyn std::error::Error>> {
    let client = Client::new();

    let variables = GetAtomsSimpleVariables {
        limit: Some(100),
        offset: Some(0),
        order_by: Some(vec![get_atoms_simple::AtomsOrderBy {
            term_id: Some(graphql_client::web::OrderBy::Desc),
        }]),
        where_: None,
    };

    let request_body = GetAtomsSimple::build_query(variables);

    let response = client
        .post(GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms_simple::ResponseData> = response.json().await?;

    Ok(data.data.map(|d| d.atoms).unwrap_or_default())
}

/// Example: Fetch atoms with detailed relationships
pub async fn fetch_atoms_with_relationships()
-> Result<Vec<get_atoms_detailed::GetAtomsDetailedAtoms>, Box<dyn std::error::Error>> {
    let client = Client::new();

    let variables = GetAtomsDetailedVariables {
        limit: Some(50),
        offset: Some(0),
        order_by: Some(vec![get_atoms_detailed::AtomsOrderBy {
            block_number: Some(graphql_client::web::OrderBy::Desc),
        }]),
        where_: None,
    };

    let request_body = GetAtomsDetailed::build_query(variables);

    let response = client
        .post(GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms_detailed::ResponseData> = response.json().await?;

    Ok(data.data.map(|d| d.atoms).unwrap_or_default())
}

/// Example: Fetch atoms filtered by type
pub async fn fetch_atoms_by_type(
    atom_type: &str,
) -> Result<Vec<get_atoms_simple::GetAtomsSimpleAtoms>, Box<dyn std::error::Error>> {
    let client = Client::new();

    let variables = GetAtomsSimpleVariables {
        limit: Some(100),
        offset: Some(0),
        order_by: None,
        where_: Some(get_atoms_simple::AtomsBoolExp {
            type_: Some(get_atoms_simple::AtomTypeComparisonExp {
                _eq: Some(atom_type.to_string()),
            }),
            ..Default::default()
        }),
    };

    let request_body = GetAtomsSimple::build_query(variables);

    let response = client
        .post(GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms_simple::ResponseData> = response.json().await?;

    Ok(data.data.map(|d| d.atoms).unwrap_or_default())
}

/// Example: Fetch atoms by creator address
pub async fn fetch_atoms_by_creator(
    creator_id: &str,
) -> Result<Vec<get_atoms_simple::GetAtomsSimpleAtoms>, Box<dyn std::error::Error>> {
    let client = Client::new();

    let variables = GetAtomsSimpleVariables {
        limit: Some(100),
        offset: Some(0),
        order_by: Some(vec![get_atoms_simple::AtomsOrderBy {
            block_number: Some(graphql_client::web::OrderBy::Desc),
        }]),
        where_: Some(get_atoms_simple::AtomsBoolExp {
            creator_id: Some(get_atoms_simple::StringComparisonExp {
                _eq: Some(creator_id.to_string()),
            }),
            ..Default::default()
        }),
    };

    let request_body = GetAtomsSimple::build_query(variables);

    let response = client
        .post(GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms_simple::ResponseData> = response.json().await?;

    Ok(data.data.map(|d| d.atoms).unwrap_or_default())
}

/// Example: Fetch atoms with full relationships (including triples)
pub async fn fetch_atoms_with_triples()
-> Result<Vec<get_atoms::GetAtomsAtoms>, Box<dyn std::error::Error>> {
    let client = Client::new();

    let variables = GetAtomsVariables {
        limit: Some(25), // Smaller limit due to complex relationships
        offset: Some(0),
        order_by: Some(vec![get_atoms::AtomsOrderBy {
            term_id: Some(graphql_client::web::OrderBy::Desc),
        }]),
        where_: None,
    };

    let request_body = GetAtoms::build_query(variables);

    let response = client
        .post(GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms::ResponseData> = response.json().await?;

    Ok(data.data.map(|d| d.atoms).unwrap_or_default())
}

/// Example: Paginate through all atoms
pub async fn fetch_all_atoms_paginated()
-> Result<Vec<get_atoms_simple::GetAtomsSimpleAtoms>, Box<dyn std::error::Error>> {
    let mut all_atoms = Vec::new();
    let mut offset = 0;
    let limit = 100;

    loop {
        let client = Client::new();

        let variables = GetAtomsSimpleVariables {
            limit: Some(limit),
            offset: Some(offset),
            order_by: Some(vec![get_atoms_simple::AtomsOrderBy {
                term_id: Some(graphql_client::web::OrderBy::Asc),
            }]),
            where_: None,
        };

        let request_body = GetAtomsSimple::build_query(variables);

        let response = client
            .post(GRAPHQL_ENDPOINT)
            .json(&request_body)
            .send()
            .await?;

        let data: graphql_client::Response<get_atoms_simple::ResponseData> =
            response.json().await?;

        let atoms = data.data.map(|d| d.atoms).unwrap_or_default();

        if atoms.is_empty() {
            break;
        }

        all_atoms.extend(atoms);
        offset += limit;

        // Safety check to prevent infinite loops
        if offset > 10000 {
            break;
        }
    }

    Ok(all_atoms)
}
