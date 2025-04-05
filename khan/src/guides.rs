/// ## Getting started
///
/// The [`Entity`](crate::Entity) trait maps a Rust type to a `MongoDB` collection,
/// providing a type-safe interface for inserting, querying, updating, and deleting
/// documents.
///
/// A type that derives [`Entity`](crate::Entity) must:
/// - implement [`Serialize`](serde::Serialize) and [`Deserialize`](serde::Deserialize)
/// - have a field named `id`, annotated with `#[serde(rename = "_id")]`
/// - use a type for `id` that can be serialized to and deserialized from
///   [`ObjectId`](mongodb::bson::oid::ObjectId).
///
/// The type of the `id` field may be just [`ObjectId`](mongodb::bson::oid::ObjectId),
/// or a newtype wrapper around it. See
/// [this note](https://docs.rs/khan/latest/khan/guides/patterns_and_recommendations/index.html#use-newtypes-for-ids)
/// for why using a newtype might be a good idea.
///
/// ### Example
///
/// ```
/// use serde::{Serialize, Deserialize};
/// use khan::Entity;
/// use mongodb::bson::oid::ObjectId;
///
/// #[derive(Serialize, Deserialize, Entity)]
/// struct User {
///   id: ObjectId,
///   name: String,
///   password: String,
/// }
/// ```
///
/// Once you derive [`Entity`](crate::Entity) for a type, Khan will map it to a `MongoDB`
/// collection. By default, the collection name is the lowercase form of the struct name
/// (e.g., `User` → `user`). You can override this using the
/// `#[entity(collection = "custom_name")]` attribute.
///
/// You can then use methods from the [`Entity`](crate::Entity),
/// [`Projection`](crate::Projection), and [`ProjectionWithId`](crate::ProjectionWithId)
/// traits to interact with the database. The `Projection` and `ProjectionWithId` traits are
/// derived automatically alongside `Entity`.
///
/// ```
/// let user = User {
///   id: ObjectId::new()
///   name: "Kit Isaev".into(),
///   password: "somepassword".into(),
/// };
///
///
/// // Equivalent to:
/// // db.user.insertOne({ _id: user.id, name: "Kit Isaev", password: "somepassword" })
/// user.insert(mongo).await?;
///
/// // Equivalent to:
/// // db.user.findOne({ _id: user.id })
/// let user = User::find_one(by_id(user.id)).await?.unwrap();
///
/// // Equivalent to:
/// // db.user.deleteOne({ _id: user.id })
/// user.remove(mongo).await?;
///
/// ```
///
/// ### Creating `Mongo`
///
/// [`Mongo`](crate::Mongo) is a lightweight wrapper around a reference to
/// [`mongodb::Database`](mongodb::Database), optionally paired with a mutable reference to
/// a [`mongodb::ClientSession`](mongodb::ClientSession) for use in
/// [transactions](super::transactions_and_locking).
///
/// It is accepted by all Khan operations and can be created from a
/// [`Database`](mongodb::Database) instance:
///
/// ```
/// let client = Client::with_uri_str("mongodb://example.com").await?;
/// let db = client.database("mydb");
/// let mongo: Mongo = db.into();
/// user.insert(mongo).await?;
/// ```
///
/// For detailed instructions on establishing a connection and creating a
/// [`Database`](mongodb::Database) instance, please refer to the
/// [`mongodb` documentation](mongodb::Client).
///
/// ### Method overview
///
/// | Method name                       | Description                                                                      | Example                                                                                               | Corresponding MongoDB Query                                                                   |  
/// |-----------------------------------|----------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|  
/// | `Entity::insert`                  | Inserts a new entity into the database.                                          | `User { id, name: "Kit".into(), password: "pass".into() }.insert(mongo).await?;`                      | `db.collection('user').insertOne({ _id: id, name: "Kit", password: "pass" });`                |  
/// | `Entity::insert_many`             | Inserts multiple entities into the database.                                     | `User::insert_many(mongo, &[User { id, name: "Kit".into(), password: "pass".into() }]).await?;`       | `db.collection('user').insertMany([{ _id: id, name: "Kit", password: "pass" }]);`             |
/// | `Entity::count`                   | Counts entities matching a filter.                                               | `User::count(mongo, user::filter! { name: "Kit" }).await?;`                                           | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Entity::exists`                  | Returns true if at least one entity matches the filter.                          | `User::exists(mongo, user::filter! { name: "Kit" }).await?;`                                          | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Projection::find`                | Finds entities based on a filter.                                                | `User::find(mongo, user::filter! { name: "Kit" }).await?;`                                            | `db.collection('user').find({ name: { $eq: "Kit" } });`                                       |  
/// | `Projection::find_one`            | Finds a single entity based on a filter.                                         | `User::find_one(mongo, by_id(id)).await?;`                                                            | `db.collection('user').findOne({ _id: { $eq: id } });`                                        |
/// | `Projection::find_with_opts`      | Finds entities with options for skip, limit, and sorting.                        | `User::find_with_opts(user::filter! { name: "Kit" }), by_id(id), Some(10), Some(20), None).await?;`   | `db.collection('user').find({ name: { $eq: "Kit" } }).skip(10).limit(20);`                    |  
/// | `Projection::find_one_and_update` | Finds and updates a single entity based on a filter.                             | `User::find_one_and_update(mongo, by_id(id), user::patch! { name: "Kit".into() }).await?;`            | `db.collection('user').findOneAndUpdate({ _id: id }, { $set: { name: "Kit" } });`             |
/// | `Entity::update`                  | Updates multiple documents based on a filter.                                    | `User::update(mongo, user::filter! { name: "Kit" }, user::patch { password: "pass".into() }).await?;` | `db.collection('user').updateMany({ name: { $eq: "Kit" } }, { $set: { password: "pass" } });` |  
/// | `Entity::update_one`              | Updates a single document based on a filter.                                     | `Entity::update_one(mongo, by_id(id), user::patch { password: "pass".into() }).await?;`               | `db.collection('user').updateOne({ _id: { $eq: id } }, { $set: { password: "pass" } });`      |
/// | `ProjectionWithId::patch`         | Applies a patch to an existing document based on its id, and updates the struct. | `user.patch(mongo, user::patch! { password: "pass".into() }).await?;`                                 | `db.collection('user').updateOne({ _id: { $eq: user.id } }, { $set: { password: "pass" } });` |
/// | `Entity::delete`                  | Deletes multiple documents based on a filter.                                    | `User::delete(mongo, user::filter! { name: "Kit" }).await?;`                                          | `db.collection('user').deleteMany({ name: { $eq: "Kit" } });`                                 |  
/// | `Entity::delete_one`              | Deletes a single document based on a filter.                                     | `Entity::delete_one(mongo, by_id(id)).await?;`                                                        | `db.collection('user').deleteOne({ _id: { $eq: id } });`                                      |  
/// | `ProjectionWithId::remove`        | Removes an existing entity from the database by id.                              | `user.remove(mongo).await?;`                                                                          | `db.collection('user').deleteOne({ _id: { $eq: user.id } });`                                 |
mod getting_started {}

/// `#[derive(Entity)]` macro
mod filters_updates_and_patches {}

mod projections {}

mod transactions_and_locking {}

mod patterns_and_recommendations {}

/// This library is called "Khan" because "Mongo" is a prefix to "Mongolia".
mod naming {}
