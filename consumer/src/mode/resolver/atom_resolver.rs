use crate::{
    error::ConsumerError,
    mode::{
        decoded::atom::atom_supported_types::AtomMetadata,
        types::{AtomUpdater, ResolverConsumerContext},
    },
};
use models::{
    atom::{Atom, AtomType},
    atom_value::AtomValue,
    book::Book,
    organization::Organization,
    person::Person,
    thing::Thing,
    traits::SimpleCrud,
};
use serde_json::Value;
use std::str::FromStr;
use tracing::warn;

/// Supported schema.org contexts
pub const SCHEMA_ORG_CONTEXTS: [&str; 4] = [
    "https://schema.org",
    "https://schema.org/",
    "http://schema.org",
    "http://schema.org/",
];

/// Resolves an IPFS URI
pub async fn try_to_resolve_ipfs_uri(
    atom_data: &str,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<Option<String>, ConsumerError> {
    // Handle IPFS URIs
    if let Some(ipfs_hash) = atom_data.strip_prefix("ipfs://") {
        if let Ok(ipfs_data) = resolver_consumer_context
            .ipfs_resolver
            .fetch_from_ipfs(ipfs_hash)
            .await
        {
            // Remove UTF-8 BOM if present
            let data = ipfs_data.replace('\u{feff}', "");
            Ok(Some(data))
        } else {
            warn!("Failed to fetch IPFS data, atom data: {}", atom_data);
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// This function tries to resolve a schema.org URL from the atom data
pub async fn try_to_resolve_schema_org_url(
    atom_data: &str,
) -> Result<Option<String>, ConsumerError> {
    // check if the atom data contains a predefine string (schema.org/something)
    if let Some(schema_org_url) = SCHEMA_ORG_CONTEXTS
        .iter()
        .find(|ctx| atom_data.starts_with(**ctx))
        .map(|ctx| atom_data[ctx.len()..].trim_start_matches('/').to_string())
    {
        Ok(Some(schema_org_url))
    } else {
        Ok(None)
    }
}

/// Tries to parse JSON and handle schema.org data
pub async fn try_to_parse_json(
    atom_data: &str,
    atom: &Atom,
    consumer_context: &impl AtomUpdater,
) -> Result<AtomMetadata, ConsumerError> {
    // TODO: What if the JSON is not valid? Should we return an error?
    // Currently, we just return unknown metadata
    if let Ok(json) = serde_json::from_str::<Value>(atom_data) {
        match json.get("@context").and_then(|c| c.as_str()) {
            Some(ctx_str) if SCHEMA_ORG_CONTEXTS.contains(&ctx_str) => {
                let metadata =
                    try_to_resolve_schema_org_properties(consumer_context, atom, &json).await?;
                Ok(metadata)
            }
            // TODO: Handle unsuported schemas
            Some(ctx_str) if !SCHEMA_ORG_CONTEXTS.contains(&ctx_str) => {
                warn!("Unsupported schema.org context: {}", ctx_str);
                Ok(AtomMetadata::unknown())
            }
            _ => {
                // TODO: Handle unknown contexts
                warn!("No @context found in JSON: {:?}", json);
                Ok(AtomMetadata::unknown())
            }
        }
    } else {
        Ok(AtomMetadata::unknown())
    }
}

/// Creates an atom value for a thing
pub async fn create_thing_atom_value(
    atom: &Atom,
    thing: &Thing,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .thing_id(thing.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for a person
pub async fn create_person_atom_value(
    atom: &Atom,
    person: &Person,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .person_id(person.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for an organization
pub async fn create_organization_atom_value(
    atom: &Atom,
    organization: &Organization,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .organization_id(organization.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for a book
pub async fn create_book_atom_value(
    atom: &Atom,
    book: &Book,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .book_id(book.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Resolves schema.org properties
async fn try_to_resolve_schema_org_properties(
    consumer_context: &impl AtomUpdater,
    atom: &Atom,
    obj: &Value,
) -> Result<AtomMetadata, ConsumerError> {
    if let Some(obj_type) = obj.get("@type").and_then(|t| t.as_str()) {
        // This will fail if the atom type is not supported, i.e. it's not part of
        // the [`AtomType`] enum.
        if let Ok(atom_type) = AtomType::from_str(obj_type) {
            match atom_type {
                AtomType::Thing => {
                    let thing = create_thing_from_obj(atom, obj)
                        .upsert(consumer_context.pool(), consumer_context.backend_schema())
                        .await?;
                    create_thing_atom_value(atom, &thing, consumer_context).await?;
                    Ok(AtomMetadata::thing(
                        thing.name.unwrap_or_default(),
                        thing.image.clone(),
                    ))
                }
                AtomType::Person => {
                    let person = create_person_from_obj(atom, obj)
                        .upsert(consumer_context.pool(), consumer_context.backend_schema())
                        .await?;
                    create_person_atom_value(atom, &person, consumer_context).await?;
                    Ok(AtomMetadata::person(
                        person.name.unwrap_or_default(),
                        person.image.clone(),
                    ))
                }
                AtomType::Organization => {
                    let organization = create_organization_from_obj(atom, obj)
                        .upsert(consumer_context.pool(), consumer_context.backend_schema())
                        .await?;
                    create_organization_atom_value(atom, &organization, consumer_context).await?;
                    Ok(AtomMetadata::organization(
                        organization.name.unwrap_or_default(),
                        organization.image.clone(),
                    ))
                }
                AtomType::Book => {
                    let book = create_book_from_obj(atom, obj)
                        .upsert(consumer_context.pool(), consumer_context.backend_schema())
                        .await?;
                    create_book_atom_value(atom, &book, consumer_context).await?;
                    Ok(AtomMetadata::book(book.name.unwrap_or_default()))
                }
                _ => {
                    warn!("Unsupported schema.org type: {}", obj_type);
                    Ok(AtomMetadata::unknown())
                }
            }
        } else {
            warn!("Unsupported schema.org type: {}", obj_type);
            Ok(AtomMetadata::unknown())
        }
    } else {
        Ok(AtomMetadata::unknown())
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
