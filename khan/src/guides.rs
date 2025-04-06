/// ## Getting started
///
/// The [`Entity`](crate::Entity) trait maps a Rust type to a `MongoDB` collection,
/// providing a type-safe interface for inserting, querying, updating, and deleting
/// documents.
///
/// A type that derives [`Entity`](crate::Entity) must:
/// - be a struct with named fields
/// - implement [`Serialize`](serde::Serialize) and [`Deserialize`](serde::Deserialize)
/// - have a field named `id`, annotated with `#[serde(rename = "_id")]`
/// - use a type for `id` that can be serialized to and deserialized from
///   [`ObjectId`](mongodb::bson::oid::ObjectId).
///
/// The type of the `id` field may be [`ObjectId`](mongodb::bson::oid::ObjectId) itself,
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
/// | Method name                       | Description                                                                      | Example                                                                                                 | Corresponding MongoDB Query                                                                   |  
/// |-----------------------------------|----------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|  
/// | `Entity::insert`                  | Inserts a new entity into the database.                                          | `User { id, name: "Kit".into(), password: "pass".into() }.insert(mongo).await?;`                        | `db.collection('user').insertOne({ _id: id, name: "Kit", password: "pass" });`                |  
/// | `Entity::insert_many`             | Inserts multiple entities into the database.                                     | `User::insert_many(mongo, &[User { id, name: "Kit".into(), password: "pass".into() }]).await?;`         | `db.collection('user').insertMany([{ _id: id, name: "Kit", password: "pass" }]);`             |
/// | `Entity::count`                   | Counts entities matching a filter.                                               | `User::count(mongo, user::filter! { name: "Kit" }).await?;`                                             | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Entity::exists`                  | Returns true if at least one entity matches the filter.                          | `User::exists(mongo, user::filter! { name: "Kit" }).await?;`                                            | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Projection::find`                | Finds entities based on a filter.                                                | `User::find(mongo, user::filter! { name: "Kit" }).await?;`                                              | `db.collection('user').find({ name: { $eq: "Kit" } });`                                       |  
/// | `Projection::find_one`            | Finds a single entity based on a filter.                                         | `User::find_one(mongo, by_id(id)).await?;`                                                              | `db.collection('user').findOne({ _id: { $eq: id } });`                                        |
/// | `Projection::find_with_opts`      | Finds entities with options for skip, limit, and sorting.                        | `User::find_with_opts(user::filter! { name: "Kit" }), by_id(id), Some(10), Some(20), None).await?;`     | `db.collection('user').find({ name: { $eq: "Kit" } }).skip(10).limit(20);`                    |  
/// | `Projection::find_one_and_update` | Finds and updates a single entity based on a filter.                             | `User::find_one_and_update(mongo, by_id(id), user::update! { name: "Kit".into() }).await?;`             | `db.collection('user').findOneAndUpdate({ _id: id }, { $set: { name: "Kit" } });`             |
/// | `Entity::update`                  | Updates multiple documents based on a filter.                                    | `User::update(mongo, user::filter! { name: "Kit" }, user::update! { password: "pass".into() }).await?;` | `db.collection('user').updateMany({ name: { $eq: "Kit" } }, { $set: { password: "pass" } });` |  
/// | `Entity::update_one`              | Updates a single document based on a filter.                                     | `Entity::update_one(mongo, by_id(id), user::update! { password: "pass".into() }).await?;`               | `db.collection('user').updateOne({ _id: { $eq: id } }, { $set: { password: "pass" } });`      |
/// | `ProjectionWithId::patch`         | Applies a patch to an existing document based on its id, and updates the struct. | `user.patch(mongo, user::update! { password: "pass".into() }).await?;`                                  | `db.collection('user').updateOne({ _id: { $eq: user.id } }, { $set: { password: "pass" } });` |
/// | `Entity::delete`                  | Deletes multiple documents based on a filter.                                    | `User::delete(mongo, user::filter! { name: "Kit" }).await?;`                                            | `db.collection('user').deleteMany({ name: { $eq: "Kit" } });`                                 |  
/// | `Entity::delete_one`              | Deletes a single document based on a filter.                                     | `Entity::delete_one(mongo, by_id(id)).await?;`                                                          | `db.collection('user').deleteOne({ _id: { $eq: id } });`                                      |  
/// | `ProjectionWithId::remove`        | Removes an existing entity from the database by id.                              | `user.remove(mongo).await?;`                                                                            | `db.collection('user').deleteOne({ _id: { $eq: user.id } });`                                 |
mod getting_started {}

/// Khan gives you an easy and type-safe way to build `MongoDB` filter and update documents
/// for your entities. This helps you avoid writing raw and loosely typed BSON by hand,
/// while keeping your code concise and readable.
///
/// ### Helper module
///
/// Every entity you define with `#[derive(Entity)]` gets a helper module named after
/// the entity (in `snake_case`). For example, the module for an entity named `User`
/// will be named `user`.
///
/// Inside that module, you’ll find:
/// - A `TypedFilter` struct for building type-safe `MongoDB` filter documents
/// - A `TypedUpdate` struct for building type-safe `MongoDB` update documents
///
/// These types are shaped after your entity, but each field is wrapped to
/// represent optionality and filter/update semantics.
///
/// - [`Field`](crate::Field) represents optionality of each field, and allows you to
///   construct partially populated documents with strong typing.
///   - [`Field::Set(value)`](crate::Field::Set) – include this field in the filter or
///     update with the given value.
///   - [`Field::Omit`](crate::Field::Omit) – exclude this field entirely from the filter
///     or update.
/// - [`FilterOperator`](crate::FilterOperator) represents a `MongoDB`
///   [comparison operator](https://www.mongodb.com/docs/manual/reference/operator/query/#comparison)
///   that should be applied to a field.
///
/// For example, for the following struct:
///
/// ```rust
/// #[derive(Entity)]
/// struct User {
///     id: ObjectId,
///     name: String,
/// }
/// ```
///
/// The following helper module will be generated:
///
/// ```
/// mod user {
///     pub struct TypedFilter {
///         id: Field<FilterOperator<ObjectId>>,
///         name: Field<FilterOperator<str>>,
///     }
///
///     impl Default for TypedFilter {
///         fn default() -> Self {
///             Self {
///                 id: Field::Omit,
///                 name: Field::Omit
///             }
///         }
///     }
///
///     pub struct TypedUpdate {
///         id: Field<ObjectId>,
///         name: Field<String>,
///     }
///
///     impl Default for TypedUpdate {
///         fn default() -> Self {
///             Self {
///                 id: Field::Omit,
///                 name: Field::Omit
///             }
///         }
///     }
/// }
/// ```
///
/// ### Using `TypedFilter` and `TypedUpdate`
///
/// You can pass `TypedFilter` and `TypedUpdate` to methods that accept
/// [`Filter<Entity>`](`crate::Filter`) and [`Update<Entity>`](`crate::Update`), such as
/// [`find`](crate::Projection::find), [`exists`](crate::Entity::exists),
/// [`update_one`](crate::Entity::update_one), and [`update`](crate::Entity::update).
///
/// ```
/// let user = User::find(mongo, user::TypedFilter {
///     name: Field::Set(FilterOperator::Eq("Kit")),
///     ..Default::default()
/// }).await?;
/// ```
///
/// Equivalent `MongoDB` query:
///
/// ```mongodb
/// db.user.findOne({ name: { $eq: "Kit" } });
/// ```
///
/// ```
/// User::update_one(mongo,
///     user::TypedFilter {
///         name: Field::Set(FilterOperator::Eq("Kit")),
///         ..Default::default()
///     },
///     user::TypedUpdate {
///         name: Field::Set("K.I.".to_string()),
///         ..Default::default()
///     }
/// ).await?;
/// ```
///
/// Equivalent `MongoDB` update:
///
/// ```mongodb
/// db.user.updateOne({ name: { $eq: "Kit" } }, { $set: { name: "K.I." } });
/// ```
///
/// ### Helper macros
///
/// To reduce boilerplate, each helper module also contains `filter!` and `update!` macros
/// that simplify the construction of `TypedFilter` and `TypedUpdate`.
///
/// ```
/// let filter = user::filter! {
///     name: "Kit"
/// };
/// ```
///
/// Expands to:
/// ```
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Eq("Kit")),
///     ..Default::default()
/// };
/// ```
///
/// By default, the `filter!` macro uses the `$eq` comparison operator. Other comparison
/// operators supported by [`FilterOperator`](crate::FilterOperator) can be specified
/// explicitly.
///
/// ```
/// let filter = user::filter! {
///     name: Ne("Kit")
/// };
/// ```
///
/// Expands to:
/// ```
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Ne("Kit")),
///     ..Default::default()
/// };
/// ```
///
/// And for updates:
/// ```
/// let update = user::update! {
///     name: "Kit".to_string()
/// };
/// ```
///
/// Expands to:
/// ```
/// let update = user::TypedUpdate {
///     name: Field::Set("Kit".to_string()),
///     ..Default::default()
/// };
/// ```
///
/// ### Untyped filters and updates
///
/// While `TypedFilter` and `TypedUpdate` are recommended in most cases
/// for type safety and clarity, some advanced `MongoDB` operators are not supported
/// by this crate. This is a deliberate design decision: Khan focuses on keeping
/// simple CRUD operations concise and type-safe, while providing escape hatches
/// for more complex use cases.
///
/// When you need to use operators that are not covered by the typed API — such as
/// `$slice`, `$elemMatch`, or computed expressions — you can construct an `UntypedFilter`
/// directly from raw BSON:
///
/// ```
/// let filter = UntypedFilter::new(bson::doc! {
///     "name": {
///         "$regex": "^Kit$"
///     }
/// });
///
/// let user = User::find(mongo, filter).await?;
/// ```
///
/// Similarly, you can use `UntypedUpdate` for expressing complex update operations
/// that go beyond basic `$set` — for example, `$push`, `$slice`, `$pop`, or updates
/// on deeply nested fields:
///
/// ```
/// let update = UntypedUpdate::new(bson::doc! {
///     "$push": {
///         "messages": { "$each": ["hi"], "$slice": -10 }
///     }
/// });
///
/// User::update_one(mongo, user::filter! { id: user_id }, update).await?;
/// ```
///
/// /// ### Columns enum
///
/// Every entity also gets a `Columns` enum generated inside its helper module. This enum
/// contains all the field names of your struct and implements `Display`, and is
/// recommended to use instead of string literals when constructing raw BSON documents.
///
/// This approach helps prevent typos and makes refactoring easier, since field names are
/// now compiler-checked.
///
/// For example, instead of writing:
///
/// ```
/// let filter = UntypedFilter::new(bson::doc! {
///     "name": { "$regex": "^Kit$" }
/// });
/// ```
///
/// You can write:
///
/// ```
/// let filter = UntypedFilter::new(bson::doc! {
///     user::Columns::Name: { "$regex": "^Kit$" }
/// });
/// ```
///
/// The enum also honors `#[serde(rename = "...")]`, so renamed fields will be mapped to
/// their correct BSON field names automatically.
///
/// ### Using [`patch`](crate::ProjectionWithId::patch) with [`UntypedUpdate`](crate::UntypedUpdate)
///
/// The [`patch`](crate::ProjectionWithId::patch) method can be used to update a document in
/// the database *and* apply the same changes to the struct in memory.
///
/// When typed updates from the helper module are used, this happens automatically.
/// However, when using raw BSON updates, we need to tell the
/// [`patch`](crate::ProjectionWithId::patch) method how to update the struct. This is done
/// by using [`UntypedUpdateApply`](crate::UntypedUpdateApply) instead of
/// [`UntypedUpdate`](crate::UntypedUpdate).
///
/// [`UntypedUpdateApply::new`](crate::UntypedUpdateApply::new) takes:
/// - a BSON update document
/// - a closure that applies the same changes to the struct in memory
///
/// Example:
///
/// ```
/// use bson::doc;
///
/// post.patch(mongo, UntypedUpdateApply::new(
///     doc! { "$pop": { "comments": 1 } },
///     |p| { p.comments.pop(); },
/// )).await?;
/// ```
///
/// This will remove the last comment from both the database and the local `post` instance.
mod filters_and_updates {}

mod projections {}

mod transactions_and_locking {}

mod patterns_and_recommendations {}

/// This library is named "Khan" because "Mongo" is a prefix to "Mongolia".
mod naming {}
