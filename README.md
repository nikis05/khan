# Khan

Khan is a `MongoDB` ORM (or, more precisely, an ODM) for Rust. It adds an entity API on top of the
underlying `MongoDB` driver, with type-safe methods to create, query, update, and delete documents,
as well as tools for maintaining consistency in multi-document transactions.
It can also manage collection indexes and validation rules in a code-first manner.

## Why Khan?

Khan is designed for applications that want Rust's type system around everyday MongoDB work
without hiding `MongoDB` itself.

- 🛡️ **Typed where repetition is costly.** Deriving [`Entity`] generates typed filters, updates,
  field names, projections, and CRUD methods from the same Serde model used for BSON.
- 🧰 **Explicit where `MongoDB` is powerful.** Raw BSON filters and updates remain deliberate escape
  hatches, while the underlying [`mongodb`] API stays available for aggregation pipelines and
  specialized operations.
- 🔄 **Transaction-aware by construction.** Entity operations work with either a database or a
  transaction context. [`DatabaseExt`] provides retry-aware transaction helpers, while [`Fence`]
  can express document-level reference requirements in function signatures.
- 🧩 **Code-first database metadata.** Optional features let entity declarations define and enforce
  indexes, query-expression validators, and MongoDB JSON Schema validation.
- 🎯 **A focused, composable API.** Khan handles common persistence and consistency concerns while
  application architecture, domain behavior, and advanced MongoDB operations remain ordinary
  Rust code.

## Example

```rust
use khan::{Entity, Selectable, SelectableWithId, by_id};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

// Define an entity
#[derive(Serialize, Deserialize, Entity, Debug, PartialEq, Eq)]
#[entity(
  skip_schema_validation,
  collection = "readme_user",
  // Define supported projections; respective structs are generated automatically.
  projections(Profile(id, email, password))
)]
struct User {
  #[serde(rename = "_id")]
  id: ObjectId,
  email: String,
  username: String,
  password: String,
}

async fn example(mongo: &mongodb::Database) -> mongodb::error::Result<()> {
  // Insert an entity into the database
  let user = User {
    id: ObjectId::new(),
    email: "mail@example.com".into(),
    username: "nikis05".into(),
    password: "somepassword".into(),
  };
  let user_id = user.id;

  user.insert(mongo).await?;
  assert_eq!(User::find_one(mongo, by_id(user_id)).await?, Some(user));

  // Query an entity by id
  let person: User = User::find_one(mongo, by_id(user_id)).await?.unwrap();
  assert_eq!(person.email, "mail@example.com");

  // Query an entity by custom fields
  let recent_user: User = User::find_one(mongo, user::filter! {
    username: "nikis05"
  }).await?.unwrap();
  assert_eq!(recent_user.id, user_id);

  // Query only the necessary fields of an entity
  // into a custom projection struct
  let user::Profile { email, password, .. } =
    user::Profile::find_one(mongo, by_id(user_id)).await?.unwrap();
  assert_eq!(email, "mail@example.com");
  assert_eq!(password, "somepassword");

  // Update an entity in the database
  User::update_one(mongo, by_id(user_id), user::update! {
    email: "new.email@example.com".into()
  }).await?;
  assert_eq!(
    User::find_one(mongo, by_id(user_id)).await?.unwrap().email,
    "new.email@example.com"
  );

  // Update an entity in the database and the corresponding struct
  let mut user = User::find_one(mongo, by_id(user_id)).await?.unwrap();
  user.patch(mongo, user::update! {
    email: "newer.email@example.com".into(),
    password: "someotherpassword".into()
  }).await?;
  assert_eq!(
    User::find_one(mongo, by_id(user_id)).await?.unwrap().password,
    "someotherpassword"
  );
  assert_eq!(user.password, "someotherpassword");

  // Delete one entity matching the filter
  let result = User::delete_one(mongo, by_id(user_id)).await?;
  assert!(result.deleted());
  assert!(User::find_one(mongo, by_id(user_id)).await?.is_none());

  // Remove a document from the database that corresponds to an instance
  let removable = User {
    id: ObjectId::new(),
    email: "remove@example.com".into(),
    username: "remove-me".into(),
    password: "temporary".into(),
  };
  let removable_id = removable.id;
  removable.insert(mongo).await?;
  removable.remove(mongo).await?;
  assert!(User::find_one(mongo, by_id(removable_id)).await?.is_none());

  Ok(())
}
```

[`DatabaseExt`]: https://docs.rs/khan/latest/khan/trait.DatabaseExt.html
[`Entity`]: https://docs.rs/khan/latest/khan/derive.Entity.html
[`Fence`]: https://docs.rs/khan/latest/khan/struct.Fence.html
[`mongodb`]: https://docs.rs/mongodb/latest/mongodb/
