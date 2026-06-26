use std::collections::HashSet;

use crate::{Entity, Mongo};
use mongodb::{
    IndexModel,
    bson::{self, Document, doc},
    error::Result,
};

#[doc(hidden)]
pub use inventory::submit as __private__inventory_submit;

#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct __PRIVATE__EntityMetadataWrapper(pub EntityMetadata);

inventory::collect!(__PRIVATE__EntityMetadataWrapper);

/// Metadata describing an entity’s collection name, indexes, and validation rules.
pub struct EntityMetadata {
    collection_name: &'static str,
    indexes_ptr: fn() -> Vec<IndexModel>,
    #[cfg(feature = "meta")]
    query_validation_ptr: fn() -> Option<Document>,
    #[cfg(feature = "schema")]
    json_schema_ptr: Option<fn(&mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema>,
}

impl EntityMetadata {
    /// Constructs metadata for a given entity type `E`.
    pub const fn of_entity<E: Entity>() -> Self {
        Self {
            collection_name: E::COLLECTION_NAME,
            indexes_ptr: E::indexes,
            query_validation_ptr: E::query_validation,
            #[cfg(feature = "schema")]
            json_schema_ptr: None,
        }
    }

    /// Constructs metadata for an entity along with its JSON Schema definition.
    ///
    /// Requires the entity to implement [`schemars::JsonSchema`] and the `schema` feature to be enabled.
    #[cfg(feature = "schema")]
    pub const fn of_entity_with_schema<E: Entity + schemars::JsonSchema>() -> Self {
        Self {
            collection_name: E::COLLECTION_NAME,
            indexes_ptr: E::indexes,
            query_validation_ptr: E::query_validation,
            json_schema_ptr: Some(E::json_schema),
        }
    }

    /// Returns the name of the `MongoDB` collection associated with the entity.
    pub fn collection_name(&self) -> &'static str {
        self.collection_name
    }

    /// Returns the list of indexes defined for the entity's collection.
    pub fn indexes(&self) -> Vec<IndexModel> {
        (self.indexes_ptr)()
    }

    /// Returns the query validation rule for the entity, if one is defined.
    pub fn query_validation(&self) -> Option<Document> {
        (self.query_validation_ptr)()
    }

    /// Returns the JSON Schema for the entity, if schema validation is enabled.
    ///
    /// ### Panics
    /// This method panics if `JsonSchema`s of any of the entities contain keywords or types unsupported by
    /// `MongoDB`, such as `$ref` or `integer`.
    #[cfg(feature = "schema")]
    pub fn json_schema(&self) -> Option<schemars::schema::Schema> {
        #[derive(Debug, Clone)]
        struct Visitor;

        impl schemars::visit::Visitor for Visitor {
            fn visit_schema_object(&mut self, schema: &mut schemars::schema::SchemaObject) {
                assert!(
                    if let Some(typ) = &schema.instance_type {
                        match typ {
                            schemars::schema::SingleOrVec::Single(typ) => {
                                **typ != schemars::schema::InstanceType::Integer
                            }
                            schemars::schema::SingleOrVec::Vec(types) => types
                                .iter()
                                .all(|typ| *typ != schemars::schema::InstanceType::Integer),
                        }
                    } else {
                        true
                    },
                    "`integer` type is not supported by MongoDB schema validation. Use `khan::types::Int` instead of std integer types"
                );
                assert!(
                    schema.reference.is_none(),
                    "`$ref` keyword is not supported by MongoDB schema validation. Make sure your entities don't contain recursive types"
                );
                assert!(
                    schema
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.default.as_ref())
                        .is_none(),
                    "`default` keyword is not supported by MongoDB schema validation"
                );
                assert!(
                    schema.format.is_none(),
                    "`format` keyword is not supported by MongoDB schema validation"
                );
                assert!(
                    schema
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.id.as_ref())
                        .is_none(),
                    "`id` keyword is not supported by MongoDB schema validation"
                );

                schemars::visit::visit_schema_object(self, schema);
            }
        }

        self.json_schema_ptr.map(|json_schema_ptr| {
            let mut generator = schemars::r#gen::SchemaSettings::default()
                .with(|s| {
                    s.inline_subschemas = true;
                })
                .into_generator();
            let mut schema = json_schema_ptr(&mut generator);
            schemars::visit::Visitor::visit_schema(&mut Visitor, &mut schema);
            schema
        })
    }
}

/// Returns an iterator over metadata for all defined entities in the crate.
pub fn entity_metadata() -> impl Iterator<Item = &'static EntityMetadata> {
    inventory::iter::<__PRIVATE__EntityMetadataWrapper>
        .into_iter()
        .map(|wrapper| &wrapper.0)
}

/// Ensures that all indexes declared via `#[entity(indexes(...))]` are present in the database.
///
/// This will create declared indexes on all known entities. Existing indexes are left unchanged, and
/// conflicting definitions will result in error.
///
/// Intended for development or simple production use. For more complex
/// scenarios (e.g. index migrations), use [`entity_metadata`] to implement a custom workflow.
pub async fn enforce_indexes(mongo: impl Mongo) -> Result<()> {
    for metadata in entity_metadata() {
        let indexes = metadata.indexes();

        if indexes.is_empty() {
            continue;
        }

        mongo
            .db()
            .collection::<Document>(metadata.collection_name())
            .create_indexes(indexes)
            .await?;
    }

    Ok(())
}

/// Applies query validation and JSON Schema validation rules for all defined entities.
///
/// This sets the validation rules for each collection based on:
/// - `#[entity(query_validation = ...)]`
/// - JSON Schema (if the `schema` feature is enabled and the entity implements `JsonSchema`)
///
/// Intended for development or simple use cases.
/// For advanced workflows or schema migrations, use [`entity_metadata`] to apply rules manually.
///
/// ### Panics
/// This function panics if `JsonSchema`s of any of the entities contain keywords or types unsupported by
/// `MongoDB`, such as `$ref` or `integer`.
pub async fn enforce_validation(mongo: impl Mongo) -> Result<()> {
    let existing_collections = mongo
        .db()
        .list_collection_names()
        .await?
        .into_iter()
        .collect::<HashSet<_>>();

    for metadata in entity_metadata() {
        let query_validator = metadata
            .query_validation()
            .map(|validation| doc! { "$expr": validation });

        #[cfg(feature = "schema")]
        let validator = {
            let schema_validator = metadata
                .json_schema()
                .map(|schema| doc! { "$jsonSchema": bson::to_bson(&schema).unwrap() });

            match (query_validator, schema_validator) {
                (Some(query_validator), Some(schema_validator)) => {
                    Some(doc! { "$and": [query_validator, schema_validator] })
                }
                (Some(query_validator), None) => Some(query_validator),
                (None, Some(schema_validator)) => Some(schema_validator),
                (None, None) => None,
            }
        };

        #[cfg(not(feature = "schema"))]
        let validator = query_validator;

        if let Some(validator) = validator {
            if existing_collections.contains(metadata.collection_name()) {
                mongo
                    .db()
                    .run_command(doc! {
                        "collMod": metadata.collection_name(),
                        "validator": validator,
                    })
                    .await?;
            } else {
                mongo
                    .db()
                    .create_collection(metadata.collection_name())
                    .validator(validator)
                    .await?;
            }
        }
    }
    Ok(())
}
