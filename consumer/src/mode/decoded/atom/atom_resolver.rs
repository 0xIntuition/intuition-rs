use crate::error::ConsumerError;
use log::warn;
use models::{
    atom::Atom, book::Book, organization::Organization, person::Person, thing::Thing,
    traits::SimpleCrud,
};
use serde_json::Value;
use sqlx::PgPool;

use super::ipfs_resolver::fetch_from_ipfs;

const SCHEMA_ORG_CONTEXTS: [&str; 4] = [
    "https://schema.org",
    "https://schema.org/",
    "http://schema.org",
    "http://schema.org/",
];

/// Resolves an IPFS URI
pub async fn try_to_resolve_ipfs_uri(atom_data: &str) -> Result<Option<String>, ConsumerError> {
    // Handle IPFS URIs
    if let Some(ipfs_hash) = atom_data.strip_prefix("ipfs://") {
        if let Ok(ipfs_data) = fetch_from_ipfs(ipfs_hash).await {
            // Remove UTF-8 BOM if present
            let data = ipfs_data.replace('\u{feff}', "");
            Ok(Some(data))
        } else {
            Err(ConsumerError::NetworkError(
                "Failed to fetch IPFS data".into(),
            ))
        }
    } else {
        Ok(None)
    }
}

/// Tries to parse JSON and handle schema.org data
pub async fn try_to_parse_json(
    atom_data: &str,
    atom: &Atom,
    pg_pool: &PgPool,
) -> Result<Option<String>, ConsumerError> {
    // TODO: What if the JSON is not valid? Should we return an error?
    let json: Value = serde_json::from_str(atom_data).map_err(|_| ConsumerError::InvalidJson)?;

    match json.get("@context").and_then(|c| c.as_str()) {
        Some(ctx_str) if SCHEMA_ORG_CONTEXTS.contains(&ctx_str) => {
            try_to_resolve_schema_org_properties(pg_pool, atom, &json).await
        }
        // TODO: Handle unsuported schemas
        Some(ctx_str) if !SCHEMA_ORG_CONTEXTS.contains(&ctx_str) => {
            warn!("Unsupported schema.org context: {}", ctx_str);
            Ok(None)
        }
        _ => {
            // TODO: Handle unknown contexts
            warn!("No @context found in JSON");
            Ok(None)
        }
    }
}
/// Resolves schema.org properties
async fn try_to_resolve_schema_org_properties(
    pg_pool: &PgPool,
    atom: &Atom,
    obj: &Value,
) -> Result<Option<String>, ConsumerError> {
    if let Some(obj_type) = obj.get("@type").and_then(|t| t.as_str()) {
        match obj_type {
            "Thing" => {
                create_thing_from_obj(atom, obj).upsert(pg_pool).await?;
                Ok(Some("https://schema.org/Thing".to_string()))
            }
            "Person" => {
                create_person_from_obj(atom, obj).upsert(pg_pool).await?;
                Ok(Some("https://schema.org/Person".to_string()))
            }
            "Organization" => {
                create_organization_from_obj(atom, obj)
                    .upsert(pg_pool)
                    .await?;
                Ok(Some("https://schema.org/Organization".to_string()))
            }
            "Book" => {
                create_book_from_obj(atom, obj).upsert(pg_pool).await?;
                Ok(Some("https://schema.org/Book".to_string()))
            }
            _ => {
                // Handle unknown types
                Ok(None)
            }
        }
    } else {
        Ok(None)
    }
}

/// Creates a Thing from a schema.org object
pub fn create_thing_from_obj(atom: &Atom, obj: &Value) -> Thing {
    Thing::builder()
        .id(atom.id.clone())
        .name(
            obj.get("name")
                .and_then(|name| name.as_str())
                .map(|string_name| string_name.to_string())
                .unwrap_or_default(),
        )
        .description(
            obj.get("description")
                .and_then(|description| description.as_str())
                .map(|string_description| string_description.to_string())
                .unwrap_or_default(),
        )
        .image(
            obj.get("image")
                .and_then(|image| image.as_str())
                .map(|string_image| string_image.to_string())
                .unwrap_or_default(),
        )
        .url(
            obj.get("url")
                .and_then(|url| url.as_str())
                .map(|string_url| string_url.to_string())
                .unwrap_or_default(),
        )
        .build()
}

/// Creates a Person from a schema.org object
pub fn create_person_from_obj(atom: &Atom, obj: &Value) -> Person {
    Person::builder()
        .id(atom.id.clone())
        .identifier(
            obj.get("identifier")
                .and_then(|identifier| identifier.as_str())
                .map(|string_identifier| string_identifier.to_string())
                .unwrap_or_default(),
        )
        .name(
            obj.get("name")
                .and_then(|name| name.as_str())
                .map(|string_name| string_name.to_string())
                .unwrap_or_default(),
        )
        .description(
            obj.get("description")
                .and_then(|description| description.as_str())
                .map(|string_description| string_description.to_string())
                .unwrap_or_default(),
        )
        .image(
            obj.get("image")
                .and_then(|image| image.as_str())
                .map(|string_image| string_image.to_string())
                .unwrap_or_default(),
        )
        .url(
            obj.get("url")
                .and_then(|url| url.as_str())
                .map(|string_url| string_url.to_string())
                .unwrap_or_default(),
        )
        .email(
            obj.get("email")
                .and_then(|email| email.as_str())
                .map(|string_email| string_email.to_string())
                .unwrap_or_default(),
        )
        .build()
}

/// Creates an Organization from a schema.org object
pub fn create_organization_from_obj(atom: &Atom, obj: &Value) -> Organization {
    Organization::builder()
        .id(atom.id.clone())
        .name(
            obj.get("name")
                .and_then(|name| name.as_str())
                .map(|string_name| string_name.to_string())
                .unwrap_or_default(),
        )
        .description(
            obj.get("description")
                .and_then(|description| description.as_str())
                .map(|string_description| string_description.to_string())
                .unwrap_or_default(),
        )
        .image(
            obj.get("image")
                .and_then(|image| image.as_str())
                .map(|string_image| string_image.to_string())
                .unwrap_or_default(),
        )
        .url(
            obj.get("url")
                .and_then(|url| url.as_str())
                .map(|string_url| string_url.to_string())
                .unwrap_or_default(),
        )
        .build()
}

/// Creates a Book from a schema.org object
pub fn create_book_from_obj(atom: &Atom, obj: &Value) -> Book {
    Book::builder()
        .id(atom.id.clone())
        .name(
            obj.get("name")
                .and_then(|name| name.as_str())
                .map(|string_name| string_name.to_string())
                .unwrap_or_default(),
        )
        .description(
            obj.get("description")
                .and_then(|description| description.as_str())
                .map(|string_description| string_description.to_string())
                .unwrap_or_default(),
        )
        .genre(
            obj.get("genre")
                .and_then(|genre| genre.as_str())
                .map(|string_genre| string_genre.to_string())
                .unwrap_or_default(),
        )
        .url(
            obj.get("url")
                .and_then(|url| url.as_str())
                .map(|string_url| string_url.to_string())
                .unwrap_or_default(),
        )
        .build()
}
