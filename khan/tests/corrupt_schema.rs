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
#[should_panic = "`integer` type is not supported by MongoDB schema validation. Use `khan::types::Int` instead of std integer types"]
fn corrupt_schema() {
    let metadata = khan::meta::entity_metadata()
        .find(|metadata| metadata.collection_name() == "user_with_corrupt_schema")
        .unwrap();

    assert!(metadata.indexes().is_empty());
    assert_eq!(metadata.query_validation(), None);

    metadata.json_schema();
}
