#![cfg(all(feature = "meta", feature = "schema"))]

use khan::Entity;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Entity, schemars::JsonSchema)]
#[entity(collection = "user_with_corrupt_schema")]
struct User3 {
    #[serde(rename = "_id")]
    id: khan::types::ObjectId,
    name: String,
    num_subscriptions: i32,
}

#[test]
#[should_panic = "`integer` type used in entity `corrupt_schema::User3` is not supported by MongoDB schema validation"]
fn corrupt_schema() {
    let metadata = khan::meta::entity_metadata()
        .find(|metadata| metadata.collection_name() == "user_with_corrupt_schema")
        .unwrap();

    assert!(metadata.indexes().is_empty());
    assert_eq!(metadata.query_validation(), None);

    metadata.json_schema();
}

#[derive(Debug, Serialize, Deserialize, Entity, schemars::JsonSchema)]
#[entity(collection = "recursive_entity_with_corrupt_schema")]
struct RecursiveEntity {
    #[serde(rename = "_id")]
    id: khan::types::ObjectId,
    child: Option<Box<RecursiveEntity>>,
}

#[test]
#[should_panic = "`$ref` keyword used in entity `corrupt_schema::RecursiveEntity` is not supported by MongoDB schema validation"]
fn recursive_schema() {
    let metadata = khan::meta::entity_metadata()
        .find(|metadata| metadata.collection_name() == "recursive_entity_with_corrupt_schema")
        .unwrap();

    metadata.json_schema();
}
