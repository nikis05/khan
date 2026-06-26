#![allow(dead_code)]

use khan::{
    Entity,
    mongodb::bson::{doc, oid::ObjectId},
};
use serde::{Deserialize, Serialize};

/// Creates a unique string for examples that share collections.
pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", ObjectId::new().to_hex())
}

/// Shared doctest user entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Entity)]
#[entity(
    skip_schema_validation,
    collection = "khan_doctest_user",
    projections(PublicProfile(id, name, avatar_url), AuthData(id, email, password))
)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    pub avatar_url: String,
    pub email: String,
    pub password: String,
    pub index: i32,
}

/// Builds a user with unique values.
pub fn user() -> User {
    let suffix = ObjectId::new().to_hex();
    User {
        id: ObjectId::new(),
        name: format!("User {suffix}"),
        avatar_url: format!("https://example.com/{suffix}.png"),
        email: format!("{suffix}@example.com"),
        password: unique("password"),
        index: 0,
    }
}

/// Shared doctest entity for metadata examples.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Entity)]
#[entity(
    skip_schema_validation,
    collection = "khan_doctest_indexed_user",
    indexes(
        email_idx(
            keys(email = 1),
            options = mongodb::options::IndexOptions::builder()
                .sparse(true)
                .build()
        )
    ),
    query_validation = doc! {
        "$gt": [ { "$strLenCP": { "$getField": indexed_user::Fields::Name } }, 2 ]
    }
)]
pub struct IndexedUser {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    pub email: String,
    pub password: String,
}

/// Shared comment type used by doctest post examples.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: ObjectId,
    pub text: String,
}

/// Shared doctest post entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Entity)]
#[entity(skip_schema_validation, collection = "khan_doctest_post")]
pub struct Post {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub text: String,
    pub comments: Vec<Comment>,
    pub comments_count: i32,
}

/// Builds a post with two comments.
pub fn post() -> Post {
    Post {
        id: ObjectId::new(),
        text: unique("post"),
        comments: vec![
            Comment {
                id: ObjectId::new(),
                text: "Comment #1".into(),
            },
            Comment {
                id: ObjectId::new(),
                text: "Comment #2".into(),
            },
        ],
        comments_count: 2,
    }
}
