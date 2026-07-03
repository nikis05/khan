#![cfg(all(feature = "meta", feature = "schema"))]

use khan::Entity;
use khan_macros::async_test;
use mongodb::{
    IndexModel,
    bson::{Document, doc, oid::ObjectId, to_bson},
    options::IndexOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::Semaphore;
use utils::get_mongo;

mod utils;

mod indexes {

    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User1 {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        password: String,
    }

    #[test]
    fn none() {
        assert!(User1::indexes().is_empty())
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        skip_schema_validation,
        indexes(
            __(keys(email = 1, password = "-1")),
            named(keys(email = 1)),
            with_options(
                keys(email = "-1", password = 1),
                options = IndexOptions::builder()
                    .sparse(true)
                    .name("overriden_name".to_owned())
                    .build()
            )
        )
    )]
    struct User2 {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        password: String,
    }

    #[test]
    fn typed() {
        let indexes = User2::indexes();

        let mut expected_indexes = [
            IndexModel::builder()
                .keys(doc! {
                    "email": 1,
                    "password": -1
                })
                .build(),
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(IndexOptions::builder().name("named".to_string()).build())
                .build(),
            IndexModel::builder()
                .keys(doc! { "email": -1, "password": 1 })
                .options(
                    IndexOptions::builder()
                        .name("with_options".to_string())
                        .sparse(true)
                        .build(),
                )
                .build(),
        ]
        .into_iter()
        .map(|expected_index| to_bson(&expected_index).unwrap());

        assert_eq!(indexes.len(), expected_indexes.len());

        assert!(expected_indexes.all(|expected_index| {
            indexes
                .iter()
                .any(|index| to_bson(&index).unwrap() == expected_index)
        }));
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        skip_schema_validation,
        indexes(__(keys(email = 1, password = "-1")),),
        untyped_indexes(
            IndexModel::builder()
                .keys(doc! { user3::Fields::Email: 1 })
                .build(),
            IndexModel::builder()
                .keys(doc! { user3::Fields::Password: 1 })
                .build()
        )
    )]
    struct User3 {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        password: String,
    }

    #[test]
    fn typed_and_untyped() {
        let indexes = User3::indexes();

        let mut expected_indexes = [
            IndexModel::builder()
                .keys(doc! {
                    "email": 1,
                    "password": -1
                })
                .build(),
            IndexModel::builder().keys(doc! { "email": 1 }).build(),
            IndexModel::builder().keys(doc! { "password": 1 }).build(),
        ]
        .into_iter()
        .map(|expected_index| to_bson(&expected_index).unwrap());

        assert_eq!(indexes.len(), expected_indexes.len());

        assert!(expected_indexes.all(|expected_index| {
            indexes
                .iter()
                .any(|index| to_bson(&index).unwrap() == expected_index)
        }));
    }
}

mod query_validation {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User1 {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
    }

    #[test]
    fn empty() {
        assert_eq!(User1::query_validation(), None);
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        skip_schema_validation,
        query_validation = doc! { "$gt": [ { "$strLenCP": { "$getField": user2::Fields::Name } }, 2 ] }
    )]
    struct User2 {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
    }

    #[test]
    fn not_empty() {
        assert_eq!(
            User2::query_validation(),
            Some(doc! { "$gt": [ { "$strLenCP": { "$getField": user2::Fields::Name } }, 2 ] })
        )
    }
}

mod entity_metadata {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        collection = "user_with_metadata",
        skip_schema_validation,
        indexes(__(keys(name = 1))),
        query_validation = doc! { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] }
    )]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
        num_subscriptions: i32,
    }

    #[test]
    fn without_schema_validation() {
        let metadata = khan::meta::entity_metadata()
            .find(|metadata| metadata.collection_name() == "user_with_metadata")
            .unwrap();

        assert_eq!(
            mongodb::bson::to_bson(&metadata.indexes()).unwrap(),
            mongodb::bson::to_bson(&vec![
                IndexModel::builder().keys(doc! { "name": 1 }).build()
            ])
            .unwrap()
        );

        assert_eq!(
            metadata.query_validation(),
            Some(doc! { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] })
        );

        assert_eq!(metadata.json_schema(), None);
    }

    #[derive(Serialize, Deserialize, Entity, schemars::JsonSchema)]
    #[entity(
        collection = "user_with_metadata_and_schema",
        indexes(__(keys(name = "-1")))
    )]
    struct User2 {
        #[serde(rename = "_id")]
        id: khan::types::ObjectId,
        name: String,
        num_subscriptions: khan::types::Int32,
    }

    #[test]
    fn with_schema_validation() {
        let metadata = khan::meta::entity_metadata()
            .find(|metadata| metadata.collection_name() == "user_with_metadata_and_schema")
            .unwrap();

        assert_eq!(
            mongodb::bson::to_bson(&metadata.indexes()).unwrap(),
            mongodb::bson::to_bson(&vec![
                IndexModel::builder().keys(doc! { "name": -1 }).build()
            ])
            .unwrap()
        );

        assert_eq!(metadata.query_validation(), None);

        assert_eq!(
            metadata.json_schema(),
            Some(
                schemars::schema::SchemaObject {
                    instance_type: Some(schemars::schema::InstanceType::Object.into()),
                    object: Some(Box::new(schemars::schema::ObjectValidation {
                        required: {
                            let mut required = BTreeSet::new();
                            required.insert("_id".into());
                            required.insert("name".into());
                            required.insert("num_subscriptions".into());
                            required
                        },
                        properties: {
                            let mut properties = BTreeMap::new();
                            properties.insert(
                                "_id".into(),
                                schemars::schema::SchemaObject {
                                    extensions: {
                                        let mut extensions = BTreeMap::new();
                                        extensions.insert("bsonType".into(), "objectId".into());
                                        extensions
                                    },
                                    ..Default::default()
                                }
                                .into(),
                            );
                            properties.insert(
                                "name".into(),
                                schemars::schema::SchemaObject {
                                    instance_type: Some(
                                        schemars::schema::InstanceType::String.into(),
                                    ),
                                    ..Default::default()
                                }
                                .into(),
                            );
                            properties.insert(
                                "num_subscriptions".into(),
                                schemars::schema::SchemaObject {
                                    extensions: {
                                        let mut extensions = BTreeMap::new();
                                        extensions.insert("bsonType".into(), "int".into());
                                        extensions
                                    },
                                    ..Default::default()
                                }
                                .into(),
                            );
                            properties
                        },
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into()
            )
        );
    }
}

mod enforce_indexes {
    use futures_util::TryStreamExt;

    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        collection = "user_with_enforced_indexes",
        skip_schema_validation,
        indexes(name(keys(name = 1)))
    )]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
    }

    #[async_test]
    async fn new_collection() {
        non_concurrently(async {
            get_mongo()
                .collection::<Document>("user_with_enforced_indexes")
                .drop()
                .await
                .unwrap();

            khan::meta::enforce_indexes(get_mongo()).await.unwrap();
        })
        .await;

        let indexes = get_mongo()
            .collection::<Document>("user_with_enforced_indexes")
            .list_indexes()
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(
            to_bson(&indexes).unwrap(),
            to_bson(&vec![
                IndexModel::builder()
                    .keys(doc! { "_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(Some("_id_".into()))
                            .version(mongodb::options::IndexVersion::V2)
                            .build()
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "name": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(Some("name".into()))
                            .version(mongodb::options::IndexVersion::V2)
                            .build()
                    )
                    .build()
            ])
            .unwrap()
        )
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        collection = "user_with_enforced_indexes_and_existing_collection",
        skip_schema_validation,
        indexes(name(keys(name = 1)))
    )]
    struct User2 {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
    }

    #[async_test]
    async fn existing_collection() {
        non_concurrently(async {
            get_mongo()
                .collection::<Document>("user_with_enforced_indexes_and_existing_collection")
                .drop()
                .await
                .unwrap();

            get_mongo()
                .create_collection("user_with_enforced_indexes_and_existing_collection")
                .await
                .unwrap();

            khan::meta::enforce_indexes(get_mongo()).await.unwrap();
        })
        .await;

        let indexes = get_mongo()
            .collection::<Document>("user_with_enforced_indexes_and_existing_collection")
            .list_indexes()
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(
            to_bson(&indexes).unwrap(),
            to_bson(&vec![
                IndexModel::builder()
                    .keys(doc! { "_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(Some("_id_".into()))
                            .version(mongodb::options::IndexVersion::V2)
                            .build()
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "name": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(Some("name".into()))
                            .version(mongodb::options::IndexVersion::V2)
                            .build()
                    )
                    .build()
            ])
            .unwrap()
        )
    }
}

mod enforce_validation {
    use futures_util::{StreamExt, TryStreamExt};

    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        collection = "user_with_enforced_validation",
        skip_schema_validation,
        query_validation = doc! { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] }
    )]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
        num_subscriptions: khan::types::Int32,
    }

    #[async_test]
    async fn new_collection() {
        non_concurrently(async {
            get_mongo()
                .collection::<Document>("user_with_enforced_validation")
                .drop()
                .await
                .unwrap();

            khan::meta::enforce_validation(get_mongo()).await.unwrap();
        })
        .await;

        let metadata = std::pin::pin!(get_mongo().list_collections().await.unwrap().try_filter(
            |spec| futures_util::future::ready(spec.name == "user_with_enforced_validation")
        ))
        .next()
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            metadata.options.validator,
            Some(
                doc! { "$expr": { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] } }
            )
        );
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        collection = "user_with_enforced_validation_and_existing_collection",
        skip_schema_validation,
        query_validation = doc! { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] }
    )]
    struct User2 {
        #[serde(rename = "_id")]
        id: ObjectId,
        name: String,
        num_subscriptions: khan::types::Int32,
    }

    #[async_test]
    async fn existing_collection() {
        non_concurrently(async {
            get_mongo()
                .collection::<Document>("user_with_enforced_validation_and_existing_collection")
                .drop()
                .await
                .unwrap();

            get_mongo()
                .create_collection("user_with_enforced_validation_and_existing_collection")
                .await
                .unwrap();

            khan::meta::enforce_validation(get_mongo()).await.unwrap();
        })
        .await;

        let metadata = std::pin::pin!(get_mongo().list_collections().await.unwrap().try_filter(
            |spec| futures_util::future::ready(
                spec.name == "user_with_enforced_validation_and_existing_collection"
            )
        ))
        .next()
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            metadata.options.validator,
            Some(
                doc! { "$expr": { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] } }
            )
        );
    }

    #[derive(Serialize, Deserialize, Entity, schemars::JsonSchema)]
    #[entity(collection = "user_with_enforced_json_schema_validation_only")]
    struct User3 {
        #[serde(rename = "_id")]
        id: khan::types::ObjectId,
        name: String,
        num_subscriptions: khan::types::Int32,
    }

    #[async_test]
    async fn json_schema_only() {
        non_concurrently(async {
            khan::meta::enforce_validation(get_mongo()).await.unwrap();
        })
        .await;

        let metadata = std::pin::pin!(get_mongo().list_collections().await.unwrap().try_filter(
            |spec| futures_util::future::ready(
                spec.name == "user_with_enforced_json_schema_validation_only"
            )
        ))
        .next()
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            metadata.options.validator,
            Some(doc! {
                "$jsonSchema": {
                    "type": "object",
                    "required": ["_id", "name", "num_subscriptions"],
                    "properties": {
                        "_id": { "bsonType": "objectId" },
                        "name": { "type": "string" },
                        "num_subscriptions": { "bsonType": "int" }
                    }
                }
            })
        );
    }

    #[derive(Serialize, Deserialize, Entity, schemars::JsonSchema)]
    #[entity(
        collection = "user_with_enforced_json_schema_and_query_validation",
        query_validation = doc! { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] }
    )]
    struct User4 {
        #[serde(rename = "_id")]
        id: khan::types::ObjectId,
        name: String,
        num_subscriptions: khan::types::Int32,
    }

    #[async_test]
    async fn query_and_json_schema() {
        non_concurrently(async {
            khan::meta::enforce_validation(get_mongo()).await.unwrap();
        })
        .await;

        let metadata = std::pin::pin!(get_mongo().list_collections().await.unwrap().try_filter(
            |spec| futures_util::future::ready(
                spec.name == "user_with_enforced_json_schema_and_query_validation"
            )
        ))
        .next()
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            metadata.options.validator,
            Some(doc! {
                "$and": [
                    {
                        "$expr": { "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ] }
                    },
                    {
                        "$jsonSchema": {
                            "type": "object",
                            "required": ["_id", "name", "num_subscriptions"],
                            "properties": {
                                "_id": { "bsonType": "objectId" },
                                "name": { "type": "string" },
                                "num_subscriptions": { "bsonType": "int" }
                            }
                        }
                    },
                ]
            })
        );
    }
}

static SEMAPHORE: Semaphore = Semaphore::const_new(1);

async fn non_concurrently<F: Future<Output = ()>>(fut: F) {
    let permit = SEMAPHORE.acquire().await.unwrap();
    fut.await;
    drop(permit);
}
