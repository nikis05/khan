use crate::Mongo;
use mongodb::{IndexModel, bson::Document, error::Result};

#[doc(hidden)]
pub struct EntityMetadataWrapper(pub EntityMetadata);

inventory::collect!(EntityMetadataWrapper);

pub struct EntityMetadata {
    collection_name: &'static str,
    indexes_ptr: fn() -> &'static [IndexModel],
    #[cfg(feature = "schema")]
    json_schema_ptr: fn(&mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema,
}

impl EntityMetadata {
    pub fn collection_name(&self) -> &'static str {
        self.collection_name
    }

    pub fn indexes(&self) -> &'static [IndexModel] {
        (self.indexes_ptr)()
    }

    #[cfg(feature = "schema")]
    pub fn json_schema(&self) -> schemars::schema::Schema {
        #[derive(Debug, Clone)]
        struct Visitor;

        impl schemars::visit::Visitor for Visitor {
            fn visit_schema_object(&mut self, schema: &mut schemars::schema::SchemaObject) {
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

                schemars::visit::visit_schema_object(self, schema);
            }
        }

        let mut generator = schemars::r#gen::SchemaGenerator::new(
            schemars::r#gen::SchemaSettings::default().with(|s| {
                s.inline_subschemas = true;
                s.visitors = vec![Box::new(Visitor)];
            }),
        );
        (self.json_schema_ptr)(&mut generator)
    }
}

pub fn entity_metadata() -> impl Iterator<Item = &'static EntityMetadata> {
    inventory::iter::<EntityMetadataWrapper>
        .into_iter()
        .map(|wrapper| &wrapper.0)
}

pub async fn enforce_indexes(mongo: Mongo<'_>) -> Result<()> {
    for metadata in entity_metadata() {
        mongo
            .db
            .collection::<Document>(metadata.collection_name())
            .create_indexes(metadata.indexes().iter().cloned())
            .await?;
    }

    Ok(())
}
