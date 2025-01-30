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
    byte_object::ByteObject,
    json_object::JsonObject,
    organization::Organization,
    person::Person,
    text_object::TextObject,
    thing::Thing,
    traits::SimpleCrud,
};
use reqwest::Response;
use serde_json::Value;
use std::str::FromStr;
use tracing::{info, warn};

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
) -> Result<Option<Response>, ConsumerError> {
    // Handle IPFS URIs
    if let Some(ipfs_hash) = atom_data.strip_prefix("ipfs://") {
        if let Ok(ipfs_data) = resolver_consumer_context
            .ipfs_resolver
            .fetch_from_ipfs(ipfs_hash)
            .await
        {
            // At this point we dont know what type of data is contained in the response,
            // we just know that the response is valid and the file exists
            Ok(Some(ipfs_data))
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

/// Determines if data is likely binary based on the proportion of non-text characters
fn is_likely_binary(data: &str) -> bool {
    let binary_threshold = 0.3; // 30% non-text chars suggests binary
    let non_text_chars = data
        .as_bytes()
        .iter()
        .filter(|&&b| b < 32 && !b.is_ascii_whitespace() || b == 127)
        .count();

    (non_text_chars as f32 / data.len() as f32) > binary_threshold
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

/// Creates a ByteObject from a schema.org object
pub fn create_byte_object_from_obj(atom: &Atom, obj: Vec<u8>) -> Result<ByteObject, ConsumerError> {
    let byte_object = ByteObject::builder().id(atom.id.clone()).data(obj).build();
    if !byte_object.data.is_empty() && byte_object.data.len() <= 1_000_000 {
        Ok(byte_object)
    } else {
        Err(ConsumerError::ByteObjectError(
            "Failed to create ByteObject".to_string(),
        ))
    }
}

/// Creates a JsonObject from a schema.org object
pub fn create_json_object_from_obj(atom: &Atom, obj: &Value) -> JsonObject {
    JsonObject::builder()
        .id(atom.id.clone())
        .data(obj.to_string())
        .build()
}

/// Creates a TextObject from a schema.org object
pub fn create_text_object_from_obj(atom: &Atom, obj: &str) -> TextObject {
    TextObject::builder().id(atom.id.clone()).data(obj).build()
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

/// Handles schema.org JSON
async fn handle_schema_org_json(
    consumer_context: &impl AtomUpdater,
    atom: &Atom,
    json: &Value,
) -> Result<AtomMetadata, ConsumerError> {
    let metadata = try_to_resolve_schema_org_properties(consumer_context, atom, json).await?;
    Ok(metadata)
}

/// Handles regular JSON
async fn handle_regular_json(
    consumer_context: &impl AtomUpdater,
    atom: &Atom,
    json: &Value,
) -> Result<AtomMetadata, ConsumerError> {
    info!(
        "No @context found in JSON: {:?}, returning it as JsonObject",
        json
    );
    let json_object = create_json_object_from_obj(atom, json)
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    create_json_object_atom_value(atom, &json_object, consumer_context).await?;
    Ok(AtomMetadata::json_object(None))
}

/// Handles binary data
async fn handle_binary_data(
    consumer_context: &impl AtomUpdater,
    atom: &Atom,
    atom_data: &str,
) -> Result<AtomMetadata, ConsumerError> {
    info!("Data is likely binary, returning it as ByteObject");
    let byte_object = create_byte_object_from_obj(atom, atom_data.as_bytes().to_vec());
    match byte_object {
        Ok(byte_object) => {
            byte_object
                .upsert(consumer_context.pool(), consumer_context.backend_schema())
                .await?;
            create_byte_object_atom_value(atom, &byte_object, consumer_context).await?;
            Ok(AtomMetadata::byte_object(None))
        }
        Err(e) => {
            warn!("Failed to create ByteObject: {}", e);
            Ok(AtomMetadata::unknown())
        }
    }
}

/// Handles text data
async fn handle_text_data(
    consumer_context: &impl AtomUpdater,
    atom: &Atom,
    atom_data: &str,
) -> Result<AtomMetadata, ConsumerError> {
    if atom_data.starts_with("ipfs://") {
        return Ok(AtomMetadata::unknown());
    }

    info!("Data is likely text, returning it as TextObject");
    let text_object = create_text_object_from_obj(atom, atom_data)
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    create_text_object_atom_value(atom, &text_object, consumer_context).await?;
    Ok(AtomMetadata::text_object(None))
}

/// Tries to parse JSON
pub async fn try_to_parse_json(
    atom_data: &str,
    atom: &Atom,
    consumer_context: &impl AtomUpdater,
) -> Result<AtomMetadata, ConsumerError> {
    if let Ok(json) = serde_json::from_str::<Value>(atom_data) {
        match json.get("@context").and_then(|c| c.as_str()) {
            Some(ctx_str) if SCHEMA_ORG_CONTEXTS.contains(&ctx_str) => {
                handle_schema_org_json(consumer_context, atom, &json).await
            }
            _ => handle_regular_json(consumer_context, atom, &json).await,
        }
    } else if is_likely_binary(atom_data) {
        handle_binary_data(consumer_context, atom, atom_data).await
    } else {
        handle_text_data(consumer_context, atom, atom_data).await
    }
}

/// Creates an atom value for a byte object
pub async fn create_byte_object_atom_value(
    atom: &Atom,
    byte_object: &ByteObject,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .account_id(atom.creator_id.clone())
        .byte_object_id(byte_object.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for a text object
pub async fn create_text_object_atom_value(
    atom: &Atom,
    text_object: &TextObject,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .account_id(atom.creator_id.clone())
        .text_object_id(text_object.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for a json object
pub async fn create_json_object_atom_value(
    atom: &Atom,
    json_object: &JsonObject,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .account_id(atom.creator_id.clone())
        .json_object_id(json_object.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}

/// Creates an atom value for a thing
pub async fn create_thing_atom_value(
    atom: &Atom,
    thing: &Thing,
    consumer_context: &impl AtomUpdater,
) -> Result<(), ConsumerError> {
    AtomValue::builder()
        .id(atom.id.clone())
        .account_id(atom.creator_id.clone())
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
        .account_id(atom.creator_id.clone())
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
        .account_id(atom.creator_id.clone())
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
        .account_id(atom.creator_id.clone())
        .book_id(book.id.clone())
        .build()
        .upsert(consumer_context.pool(), consumer_context.backend_schema())
        .await?;
    Ok(())
}
