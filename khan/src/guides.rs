/// # Getting started
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
///   [`ObjectId`](mongodb::bson::oid::ObjectId), and implement `Clone + Send + 'static`.
///
/// The type of the `id` field may be [`ObjectId`](mongodb::bson::oid::ObjectId) itself,
/// or a newtype wrapper around it. See
/// [this note](https://docs.rs/khan/latest/khan/guides/c5_patterns_and_recommendations/index.html#use-newtypes-for-ids)
/// on why using a newtype might be a good idea.
///
/// ## Example
///
/// ```ignore
/// use serde::{Serialize, Deserialize};
/// use khan::Entity;
/// use mongodb::bson::oid::ObjectId;
///
/// #[derive(Serialize, Deserialize, Entity)]
/// struct User {
///   #[serde(rename = "_id")]
///   id: ObjectId,
///   name: String,
///   password: String,
/// }
///
/// assert_eq!(User::COLLECTION_NAME, "user");
/// ```
///
/// Once you derive [`Entity`](crate::Entity) for a type, Khan will map it to a `MongoDB`
/// collection. By default, the collection name is the `snake_case` form of the struct name
/// (e.g., `AuditLog` becomes `audit_log`). You can override this using the
/// `#[entity(collection = "custom_name")]` attribute.
///
/// You can then use methods from the [`Entity`](crate::Entity),
/// [`Selectable`](crate::Selectable), and [`SelectableWithId`](crate::SelectableWithId)
/// traits to interact with the database. The `Selectable` and `SelectableWithId` traits are
/// derived automatically alongside `Entity`.
///
/// ```
/// # use khan::{Entity, Selectable, SelectableWithId, by_id};
/// # #[path = "test_support.rs"] mod test_support;
/// # use test_support::{RUNTIME, User, mongo};
/// # use mongodb::bson::oid::ObjectId;
/// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
/// let user = User {
///   id: ObjectId::new(),
/// #   email: test_support::unique("john@example.com"),
/// #   avatar_url: "https://example.com/john.png".into(),
///   name: "John Doe".into(),
///   password: "somepassword".into(),
/// #   index: 0.into(),
/// };
/// # let user_id = user.id;
///
///
/// // Equivalent to:
/// // db.user.insertOne({ _id: user.id, name: "John Doe", password: "somepassword" })
/// user.insert(mongo).await?;
///
/// // Equivalent to:
/// // db.user.findOne({ _id: user.id })
/// let user = User::find_one(mongo, by_id(user_id)).await?.unwrap();
/// assert_eq!(user.id, user_id);
///
/// // Equivalent to:
/// // db.user.deleteOne({ _id: user.id })
/// let result = user.remove(mongo).await?;
/// assert!(result.deleted());
///
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
/// ## Supplying `Mongo`
///
/// [`Mongo`](crate::Mongo) is the trait Khan uses for database access. It is
/// implemented for [`&Database`](mongodb::Database), optionally paired with a
/// mutable reference to a [`mongodb::ClientSession`] for use in
/// [transactions](c4_transactions_and_fencing).
///
/// It is accepted by all Khan operations and can be supplied from a
/// [`Database`](mongodb::Database) instance:
///
/// ```ignore
/// let client = Client::with_uri_str("mongodb://example.com").await?;
/// let db = client.database("mydb");
/// user.insert(&db).await?;
/// ```
///
/// For detailed instructions on establishing a connection and creating a
/// [`Database`](mongodb::Database) instance, please refer to the
/// [`mongodb` documentation](mongodb::Client).
///
/// Because [`&Database`](mongodb::Database) is reusable, normal database operations can
/// pass the same reference repeatedly:
///
/// ```
/// # use khan::{Entity, Selectable, SelectableWithId};
/// # #[path = "test_support.rs"] mod test_support;
/// # use test_support::{RUNTIME, User, mongo, user};
/// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
/// # let email = test_support::unique("john@example.com");
/// # let mut seed = test_support::user();
/// # seed.email = email.clone();
/// # seed.insert(mongo).await?;
///
/// let user = User::find_one(mongo, user::filter! {
///     email: &email
/// }).await?;
///
/// if let Some(user) = user {
///     let result = user.remove(mongo).await?;
///     assert!(result.deleted());
/// }
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
/// ## Method overview
///
/// | Method name                       | Description                                                                      | Example                                                                                                 | Corresponding MongoDB Query                                                                     |
/// |-----------------------------------|----------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------|
/// | `Entity::insert`                  | Inserts a new entity into the database.                                          | `User { id, name: "John".into(), password: "pass".into() }.insert(mongo).await?;`                        | `db.collection('user').insertOne({ _id: id, name: "John", password: "pass" });`                |
/// | `Entity::insert_many`             | Inserts multiple entities into the database.                                     | `User::insert_many(mongo, &[User { id, name: "John".into(), password: "pass".into() }]).await?;`         | `db.collection('user').insertMany([{ _id: id, name: "John", password: "pass" }]);`             |
/// | `Entity::count`                   | Counts entities matching a filter.                                               | `User::count(mongo, user::filter! { name: "John" }).await?;`                                             | `db.collection('user').count({ name: { $eq: "John" } });`                                      |
/// | `Entity::exists`                  | Returns true if at least one entity matches the filter.                          | `User::exists(mongo, user::filter! { name: "John" }).await?;`                                            | `db.collection('user').count({ name: { $eq: "John" } });`                                      |
/// | `Selectable::find`                | Finds entities based on a filter.                                                | `User::find(mongo, user::filter! { name: "John" }).await?;`                                              | `db.collection('user').find({ name: { $eq: "John" } });`                                       |
/// | `Selectable::find_one`            | Finds a single entity based on a filter.                                         | `User::find_one(mongo, by_id(id)).await?;`                                                              | `db.collection('user').findOne({ _id: { $eq: id } });`                                          |
/// | `Selectable::find_with_opts`      | Finds entities with options for skip, limit, and sorting.                        | `User::find_with_opts(mongo, user::filter! { name: "John" }, FindOptions::new().skip(10).limit(20)).await?;` | `db.collection('user').find({ name: { $eq: "John" } }).skip(10).limit(20);`                |
/// | `Selectable::find_one_and_update` | Finds and updates a single entity, returning the pre-update document.            | `User::find_one_and_update(mongo, by_id(id), user::update! { name: "John".into() }).await?;`             | `db.collection('user').findOneAndUpdate({ _id: id }, { $set: { name: "John" } });`             |
/// | `Entity::update`                  | Updates multiple documents based on a filter.                                    | `User::update(mongo, user::filter! { name: "John" }, user::update! { password: "pass".into() }).await?;` | `db.collection('user').updateMany({ name: { $eq: "John" } }, { $set: { password: "pass" } });` |
/// | `Entity::update_one`              | Updates a single document based on a filter.                                     | `Entity::update_one(mongo, by_id(id), user::update! { password: "pass".into() }).await?;`               | `db.collection('user').updateOne({ _id: { $eq: id } }, { $set: { password: "pass" } });`        |
/// | `SelectableWithId::patch`         | Applies a patch to an existing document based on its id, and updates the struct. | `user.patch(mongo, user::update! { password: "pass".into() }).await?;`                                  | `db.collection('user').updateOne({ _id: { $eq: user.id } }, { $set: { password: "pass" } });`   |
/// | `Entity::delete`                  | Deletes multiple documents based on a filter.                                    | `User::delete(mongo, user::filter! { name: "John" }).await?;`                                            | `db.collection('user').deleteMany({ name: { $eq: "John" } });`                                 |
/// | `Entity::delete_one`              | Deletes a single document based on a filter.                                     | `Entity::delete_one(mongo, by_id(id)).await?;`                                                          | `db.collection('user').deleteOne({ _id: { $eq: id } });`                                        |
/// | `SelectableWithId::remove`        | Removes an existing entity from the database by id.                              | `user.remove(mongo).await?;`                                                                            | `db.collection('user').deleteOne({ _id: { $eq: user.id } });`                                   |
pub mod c1_getting_started {}

/// # Filters and updates
///
/// `khan` gives you an easy and type-safe way to build `MongoDB` filter and update documents
/// for your entities. This helps you avoid writing raw and loosely typed BSON by hand,
/// while keeping your code concise and readable.
///
/// ## Helper module
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
/// ```ignore
/// use khan::Entity;
/// use mongodb::bson::oid::ObjectId;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Entity)]
/// #[entity(skip_schema_validation)]
/// struct User {
///     #[serde(rename = "_id")]
///     id: ObjectId,
///     name: String,
/// }
///
/// assert_eq!(User::FIELDS, None);
/// ```
///
/// The following helper module will be generated:
///
/// ```ignore
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
/// ## Using `TypedFilter` and `TypedUpdate`
///
/// You can pass `TypedFilter` and `TypedUpdate` to methods that accept
/// [`Filter<Entity>`](`crate::Filter`) and [`Update<Entity>`](`crate::Update`), such as
/// [`find`](crate::Selectable::find), [`exists`](crate::Entity::exists),
/// [`update_one`](crate::Entity::update_one), and [`update`](crate::Entity::update).
///
/// ```
/// # use khan::{Field, Filter, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Eq("John")),
///     ..Default::default() // or `id: Field::Omit`
/// };
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$eq": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Equivalent `MongoDB` query:
///
/// ```mongodb
/// db.user.findOne({ name: { $eq: "John" } });
/// ```
///
/// ```
/// # use khan::{Field, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::TypedUpdate {
///     name: Field::Set("J.D.".to_string()),
///     ..Default::default() // or `id: Field::Omit`
/// };
///
/// assert_eq!(update.to_document()?, doc! { "$set": { "name": "J.D." } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Equivalent `MongoDB` update:
///
/// ```mongodb
/// db.user.updateOne({ name: { $eq: "John" } }, { $set: { name: "J.D." } });
/// ```
///
/// ### Helper macros
///
/// To reduce boilerplate, each helper module also contains `filter!` and `update!` macros
/// that simplify the construction of `TypedFilter` and `TypedUpdate`.
///
/// ```
/// # use khan::{Filter, Selectable};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::filter! {
///     name: "John"
/// };
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$eq": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Filter, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Eq("John")),
///     ..Default::default()
/// };
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$eq": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// By default, the `filter!` macro uses the `$eq` comparison operator. Other comparison
/// operators supported by [`FilterOperator`](crate::FilterOperator) can be specified
/// explicitly.
///
/// ```
/// # use khan::{Filter, Selectable};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::filter! {
///     name: Ne("John")
/// };
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$ne": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Filter, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Ne("John")),
///     ..Default::default()
/// };
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$ne": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// And for updates:
/// ```
/// # use khan::{Selectable, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::update! {
///     name: "John".to_string()
/// };
///
/// assert_eq!(update.to_document()?, doc! { "$set": { "name": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::TypedUpdate {
///     name: Field::Set("John".to_string()),
///     ..Default::default()
/// };
///
/// assert_eq!(update.to_document()?, doc! { "$set": { "name": "John" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// ## Untyped filters and updates
///
/// While `TypedFilter` and `TypedUpdate` are recommended in most cases
/// for type safety and clarity, some advanced `MongoDB` operators are not supported
/// by this crate. This is a deliberate design decision: `khan` focuses on keeping
/// simple CRUD operations concise and type-safe, while providing escape hatches
/// for more complex use cases.
///
/// When you need to use operators that are not covered by the typed API — such as
/// `$slice`, `$elemMatch`, or computed expressions — you can construct an `UntypedFilter`
/// directly from raw BSON:
///
/// ```
/// # use khan::{Filter, UntypedFilter};
/// # use khan::mongodb::bson::{self, doc};
/// #[path = "test_entities.rs"] mod test_support;
/// # use test_support::User;
/// let filter: UntypedFilter<User> = UntypedFilter::new(bson::doc! {
///     "name": {
///         "$regex": "^John$"
///     }
/// });
///
/// assert_eq!(filter.to_document()?, doc! { "name": { "$regex": "^John$" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// Similarly, you can use `UntypedUpdate` for expressing complex update operations
/// that go beyond basic `$set` — for example, `$push`, `$slice`, `$pop`, or updates
/// on deeply nested fields:
///
/// ```
/// # use khan::{UntypedUpdate, Update};
/// # use khan::mongodb::bson::{self, oid::ObjectId};
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::{Post, Comment};
/// # let comment = bson::to_bson(&Comment {
/// #     id: ObjectId::new(),
/// #     text: "hi".into(),
/// # }).unwrap();
/// let update: UntypedUpdate<Post> = UntypedUpdate::new(bson::doc! {
///     "$push": {
///         "comments": { "$each": [comment.clone()], "$slice": -10 }
///     }
/// });
///
/// assert_eq!(
///     update.to_document()?,
///     bson::doc! { "$push": { "comments": { "$each": [comment], "$slice": -10 } } }
/// );
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// ### `Fields` enum
///
/// Every entity also gets a `Fields` enum generated inside its helper module. This enum
/// contains all the field names of your struct and implements `Display`, and is
/// recommended to use instead of string literals when constructing raw BSON documents.
///
/// This approach helps prevent typos and makes refactoring easier, since field names are
/// now compiler-checked.
///
/// For example, instead of writing:
///
/// ```
/// # use khan::{Filter, UntypedFilter};
/// # use khan::mongodb::bson;
/// let filter: UntypedFilter<()> = UntypedFilter::new(bson::doc! {
///     "name": { "$regex": "^John$" }
/// });
///
/// assert_eq!(filter.to_document()?, bson::doc! { "name": { "$regex": "^John$" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// You can write:
///
/// ```
/// # use khan::{Filter, UntypedFilter};
/// # use khan::mongodb::bson;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter: UntypedFilter<()> = UntypedFilter::new(bson::doc! {
///     user::Fields::Name: { "$regex": "^John$" }
/// });
///
/// assert_eq!(filter.to_document()?, bson::doc! { "name": { "$regex": "^John$" } });
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// The enum also honors `#[serde(rename = "...")]`, so renamed fields will be mapped to
/// their correct BSON field names automatically.
///
/// ### Using [`patch`](crate::SelectableWithId::patch) with [`UntypedUpdate`](crate::UntypedUpdate)
///
/// The [`patch`](crate::SelectableWithId::patch) method can be used to update a document in
/// the database *and* apply the same changes to the struct in memory.
///
/// When typed updates from the helper module are used, this happens automatically.
/// However, when using raw BSON updates, we need to tell the
/// [`patch`](crate::SelectableWithId::patch) method how to update the struct. This is done
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
/// # use khan::{Entity, Selectable, SelectableWithId, UntypedUpdateApply};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_support.rs"] mod test_support;
/// # use test_support::{Post, RUNTIME, mongo};
/// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
/// # let mut post = test_support::post();
/// # post.insert(mongo).await?;
/// let result = post.patch(mongo, UntypedUpdateApply::new(
///     doc! { "$pop": { "comments": 1 } },
///     |p: &mut Post| { p.comments.pop(); },
/// )).await?;
///
/// assert!(result.matched());
/// assert_eq!(post.comments.len(), 1);
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
/// This will remove the last comment from both the database and the local `post` instance.
pub mod c2_filters_and_updates {}

/// # Projections
///
/// `MongoDB` supports selecting only specific fields from documents using projections.
/// `khan` supports this feature through the `#[entity(projections)]` attribute.
///
/// To define projections for an entity, declare them as part of the attribute:
///
/// ```ignore
/// use khan::Entity;
/// use mongodb::bson::oid::ObjectId;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Entity)]
/// #[entity(skip_schema_validation, collection = "guide_projection_user", projections(
///     PublicProfile(id, name, avatar_url),
///     AuthData(id, email, password)
/// ))]
/// struct User {
///     #[serde(rename = "_id")]
///     id: ObjectId,
///     name: String,
///     avatar_url: String,
///     email: String,
///     password: String,
/// }
///
/// assert_eq!(user::PublicProfile::FIELDS, Some(["_id", "name", "avatar_url"].as_ref()));
/// assert_eq!(user::AuthData::FIELDS, Some(["_id", "email", "password"].as_ref()));
/// ```
///
/// This will generate two additional structs inside the `user` helper module:
/// - `user::PublicProfile` containing `id`, `name`, and `avatar_url`
/// - `user::AuthData` containing `id`, `email`, and `password`
///
/// These projection structs implement the [`Selectable`](crate::Selectable) trait, and
/// support common query methods such as:
/// - `find`
/// - `find_with_opts`
/// - `find_one`
/// - `find_one_and_update`
///
/// Their projection document is generated from the fields declared in the
/// `#[entity(projections(...))]` attribute:
///
/// ```
/// # use khan::Selectable;
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let projection = user::PublicProfile::projection();
///
/// assert_eq!(projection, Some(doc! { "name": 1, "avatar_url": 1 }));
/// ```
///
/// Khan uses that projection when selecting a `PublicProfile` from the `user`
/// collection, so the returned value only includes `id`, `name`, and `avatar_url`
/// fields.
///
/// If a projection includes the `id` field, it also implements
/// [`SelectableWithId`](crate::SelectableWithId), and its instances support `remove` and
/// `patch` methods:
///
/// ```
/// # use khan::{Entity, Selectable, SelectableWithId};
/// # #[path = "test_support.rs"] mod test_support;
/// # use test_support::{RUNTIME, mongo, user};
/// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
/// # let name = test_support::unique("John");
/// # let mut seed = test_support::user();
/// # seed.name = name.clone();
/// # seed.insert(mongo).await?;
/// let mut profile = user::PublicProfile::find_one(mongo, user::filter! {
///     name: &name
/// }).await?.unwrap();
///
/// let result = profile.patch(mongo, user::update! { name: "Tom".into() }).await?;
/// assert!(result.matched());
/// assert_eq!(&profile.name, "Tom");
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
pub mod c3_projections {}

/// # Transactions and fencing
///
/// All methods on [`Entity`](crate::Entity), [`Selectable`](crate::Selectable), and
/// [`SelectableWithId`](crate::SelectableWithId) can be run in the context of a
/// transaction. Transactions can be started using either the regular
/// [`mongodb` crate API](mongodb::ClientSession) or Khan's [`DatabaseExt`](crate::DatabaseExt)
/// helpers.
///
/// When using the driver API directly, construct a [`Mongo`](crate::Mongo) instance using
/// `(&Database, &mut ClientSession)` instead of just `&Database`:
///
/// ```ignore
/// let client = Client::with_uri_str("mongodb://localhost:27017").await?;
/// let db = client.database("mydb");
///
/// let mut session = client.start_session().await?;
///
/// session.start_transaction().and_run(&db, |session| async move {
///     let mut mongo = (db, session);
///
///     let user = User::find_one(&mut mongo, user::filter! {
///         email: "john@example.com"
///     }).await?;
///
///     if let Some(user) = user {
///         user.remove(&mut mongo).await?;
///     }
///
///     Ok(())
/// }).await?;
/// ```
///
/// As a more concise alternative, [`DatabaseExt::run_transaction2`](crate::DatabaseExt::run_transaction2)
/// creates the session and supplies the transaction context:
///
/// ```ignore
/// use khan::prelude::*;
///
/// db.run_transaction2(async |mut mongo| {
///     let user = User::find_one(&mut mongo, user::filter! {
///         email: "john@example.com"
///     }).await?;
///
///     if let Some(user) = user {
///         user.remove(&mut mongo).await?;
///     }
///
///     Ok(())
/// }).await?;
/// ```
///
/// Use [`DatabaseExt::run_transaction`](crate::DatabaseExt::run_transaction) when values must be passed to
/// the callback on every transaction attempt. The callback is retried with a mutable reference to its context:
///
/// ```ignore
/// use futures_util::FutureExt;
/// use khan::prelude::*;
///
/// db.run_transaction("john@example.com".to_owned(), |mut mongo, email| async move {
///     let user = User::find_one(&mut mongo, user::filter! {
///         email: email.as_str()
///     }).await?;
///
///     if let Some(user) = user {
///         user.remove(&mut mongo).await?;
///     }
///
///     Ok(())
/// }.boxed()).await?;
/// ```
///
/// The fencing examples below use the driver syntax to show the session lifecycle explicitly, but either
/// [`DatabaseExt`](crate::DatabaseExt) helper can be used instead.
///
/// ## Fencing
///
/// Sometimes you want to make sure that a transaction does not commit based on a stale reference.
/// For example, imagine you're inserting a `Comment` that references an existing `Post` by its ID.
/// You check that the referenced post exists at the beginning of the transaction, and want to make
/// sure the comment does not commit if that post is concurrently deleted or meaningfully modified.
///
/// The following code only checks existence. It does not write to `Post`, so `MongoDB` has no
/// document-level write conflict to detect for the reference:
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session);
///
///             if !Post::exists(&mut mongo, by_id(post_id)).await? {
///                 return Err(Error::custom("Post is not found"));
///             }
///
///             // Post may be deleted between these two operations,
///             // rendering a reference by ID invalid.
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(&mut mongo)
///             .await?;
///
///             Ok(())
///         },
///     )
///     .await?;
/// ```
///
/// If the transaction already performs a meaningful update to the document (for example,
/// if adding a comment increments the `commentsCount` field on `Post`), no additional steps
/// are required to guard against conflicts. The update itself establishes document-level
/// conflict detection.
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session);
///
///             // This update acts as a fence by modifying the document.
///             let result = Post::update_one(
///                 &mut mongo,
///                 by_id(post_id),
///                 UntypedUpdate::new(doc! {
///                     "$inc": { "commentsCount": 1 }
///                 }),
///             )
///             .await?;
///
///             if !result.matched() {
///                 return Err(Error::custom("Post is not found"));
///             }
///
///             // If another writer changes the post first, this transaction aborts or retries.
///             // Once this write succeeds, another writer cannot change the post until
///             // this transaction completes.
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(&mut mongo)
///             .await?;
///
///             Ok(())
///         },
///     )
///     .await?;
/// ```
///
/// However, if no meaningful changes are required, you can perform a small write to
/// trigger `MongoDB`'s document-level conflict detection:
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session);
///
///             // We're not making any meaningful changes to the Post, but we still want
///             // concurrent modifications/deletes to conflict with this transaction.
///             let result = Post::update_one(
///                 &mut mongo,
///                 by_id(post_id),
///                 UntypedUpdate::new(doc! {
///                     "$inc": { "__fence": 1 }
///                 }),
///             )
///             .await?;
///
///             if !result.matched() {
///                 return Err(Error::custom("Post is not found"));
///             }
///
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(&mut mongo)
///             .await?;
///
///             Ok(())
///         },
///     )
///     .await?;
/// ```
///
/// This write establishes an order between the transaction and other writes to the same document.
/// If another write reaches the document first, the transaction aborts because of a write conflict.
/// Once the write succeeds, a competing writer cannot change the document until the transaction
/// completes: it either waits or fails with a write conflict. In either case, the transaction cannot
/// commit based on document state that another writer changed before the dummy write was made.
///
/// Khan refers to this technique as "fencing" and provides methods that intentionally write to a
/// dummy field named `__fence`. For example, [`find_one_and_fence`](crate::Selectable::find_one_and_fence)
/// finds a document and increments the dummy field.
///
/// This technique works well when the entire transaction happens within a single method or scope.
///
/// However, if a transaction spans multiple methods, it can become difficult to track which
/// documents have been fenced and which haven’t. This makes it easy to accidentally skip a
/// necessary fence, leading to race conditions or inconsistent state:
///
/// ```ignore
/// // Model code
/// async fn create_post(trx: impl Transaction, text: String) -> Result<ObjectId> {
///     let id = ObjectId::new();
///
///     // Insert a new post document into the database.
///     Post {
///         id,
///         text,
///     }
///     .insert(trx)
///     .await?;
///
///     Ok(id)
/// }
///
/// // Model code
/// async fn create_comment(trx: impl Transaction, post_id: ObjectId, text: String) -> Result<()> {
///     // Insert a new comment referencing the given post_id.
///     Comment {
///         id: ObjectId::new(),
///         post_id,
///         text,
///     }
///     .insert(trx)
///     .await?;
///
///     Ok(())
/// }
///
/// // Controller code
/// async fn make_post_with_initial_comment(ctx: AppContext, post_text: String, comment_text: String) -> Result<()> {
///     ctx.mongo().run_transaction((post_text, comment_text), |trx, (post_text, comment_text)| async move {
///         let mut trx = trx;
///
///         // The post is created as part of this transaction...
///         let post_id = create_post(&mut trx, post_text).await?;
///
///         // ...and the comment referencing it is inserted within the same transaction.
///         // This is safe because the post was inserted in this transaction.
///         create_comment(&mut trx, post_id, comment_text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
///
/// // Controller code
/// async fn make_comment(ctx: AppContext, post_id: ObjectId, text: String) -> Result<()> {
///     ctx.mongo().run_transaction((post_id, text), |trx, (post_id, text)| async move {
///         let mut trx = trx;
///
///         // This is NOT safe: we assume the post exists,
///         // but there's no guarantee it won't be deleted before the transaction commits.
///         // This can result in a comment pointing to a non-existent post.
///         create_comment(&mut trx, post_id, text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
/// ```
///
/// In these cases, it may be desirable to encode the fencing requirement in the type system.
///
/// Khan provides a [`Fence<T>`](crate::Fence) wrapper type to express this requirement explicitly in your
/// method signatures. When a value is wrapped in [`Fence<T>`](crate::Fence), it signals that the document has
/// already been inserted or written in the current transaction.
///
/// You can then require a [`Fence<T>`](crate::Fence) as input to any method that assumes the document has
/// been fenced:
///
/// ```
/// # use khan::{Entity, Fence};
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::post;
/// let fenced_post = Fence::new_unchecked(post());
/// let fenced_id = fenced_post.fenced_id();
///
/// assert_eq!(*fenced_id, fenced_post.id);
/// ```
///
/// ```ignore
/// // Model code
/// async fn create_post(
///     trx: impl Transaction,
///     text: String,
/// ) -> Result<Fence<ObjectId>> {
///     // This function proves at the type level that the post it creates
///     // has been inserted in this transaction.
///
///     let id = ObjectId::new();
///
///     // Insert the post while marking it as fenced within the transaction.
///     let post: Fence<Post> = Post {
///         id,
///         text,
///     }
///     .insert_and_fence(trx)
///     .await?;
///
///     // Convert the fenced post into a fenced ID, so we can pass it to other methods.
///     let fenced_id: Fence<ObjectId> = post.fenced_id();
///
///     Ok(fenced_id)
/// }
///
/// // Model code
/// async fn create_comment(
///     trx: impl Transaction,
///     post_id: Fence<ObjectId>, // Enforces that the referenced post is already fenced
///     text: String,
/// ) -> Result<()> {
///     // This function requires, at the type level, that the referenced post has been fenced.
///     let post_id = post_id.into_inner();
///
///     Comment {
///         id: ObjectId::new(),
///         post_id,
///         text,
///     }
///     .insert(trx)
///     .await?;
///
///     Ok(())
/// }
///
/// // Model code
/// async fn reference_post(trx: impl Transaction, post_id: ObjectId) -> Result<Fence<ObjectId>> {
///     // Attempts to fence the post by ID, returning a fenced ID if successful.
///     // If the post does not exist, returns an error.
///     match Post::fence_by_id(trx, post_id).await? {
///         Some(fenced_id) => Ok(fenced_id),
///         None => Err(Error::custom("Post with this id was not found")),
///     }
/// }
///
/// // Controller code
/// async fn make_post_with_initial_comment(
///     ctx: AppContext,
///     post_text: String,
///     comment_text: String,
/// ) -> Result<()> {
///     ctx.mongo().run_transaction((post_text, comment_text), |trx, (post_text, comment_text)| async move {
///         let mut trx = trx;
///
///         // Creates a new post - it is fenced since it has just been inserted.
///         let post_id: Fence<ObjectId> = create_post(&mut trx, post_text).await?;
///
///         // Since the post is fenced, we can insert a comment through the fenced API.
///         create_comment(&mut trx, post_id, comment_text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
///
/// // Controller code
/// async fn make_comment(ctx: AppContext, post_id: ObjectId, text: String) -> Result<()> {
///     ctx.mongo().run_transaction((post_id, text), |trx, (post_id, text)| async move {
///         let mut trx = trx;
///
///         // Ensure that the post exists and is fenced before proceeding.
///         let post_id: Fence<ObjectId> = reference_post(&mut trx, post_id).await?;
///
///         // Now that the fencing requirement is enforced by the type system,
///         // `create_comment` cannot be called unless the post is fenced.
///         create_comment(&mut trx, post_id, text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
/// ```
///
/// **Important:** a fence is not a mutex or SQL-style row lock. It works by performing a write
/// to the referenced document inside the transaction so that `MongoDB` can detect conflicting
/// concurrent writes. It does not provide predicate or range locking.
///
/// - A fence is document-scoped; it does not protect predicates, ranges, or arbitrary queries.
/// - Fencing relies on `MongoDB`'s document-level transaction locking and conflict detection.
///   A non-transactional write to the same document can also conflict with or wait for the fenced
///   transaction.
/// - `Fence<T>` does not encode the transaction that created it. It is valid only for operations
///   performed within that same transaction and must not be reused in another transaction.
/// - The transaction closure may run multiple times because the driver may retry it.
///
/// You should use this technique sparingly. Frequent write conflicts and transaction
/// retries can lead to degraded performance. Instead, consider:
///
/// - designing your schema such that related data is stored in the same document;
/// - designing your app in a way that can work around inconsistency across documents;
/// - implementing eventual consistency workflows, for example, workers that asynchronously update
///   all related documents after a document has been updated.
pub mod c4_transactions_and_fencing {}

/// # Patterns and Recommendations
///
/// Khan is designed to be flexible and unobtrusive — you can structure your application however you like. That
/// said, there are a few patterns that can help you get the most out of Khan by taking advantage of Rust's
/// strong type system and module organization.
///
/// These recommendations aim to improve clarity, reduce bugs, and make your codebase easier to maintain as it
/// grows.
///
/// ## 1. Use newtypes instead of `ObjectId`
///
/// Instead of using `ObjectId` directly in your entities, define newtype wrappers:
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
/// #[serde(transparent)]
/// pub struct PostId(pub ObjectId);
/// ```
///
/// This helps avoid mixing up IDs of different entities and improves type safety across your codebase.
///
/// You can use this newtype as an `Entity::Id` type, and Khan will handle it like a regular `ObjectId`.
///
/// **NOT RECOMMENDED:**
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Entity)]
/// struct Post {
///     id: ObjectId,
///     text: String,
/// }
///
/// #[derive(Serialize, Deserialize, Entity)]
/// struct Comment {
///     id: ObjectId,
///     post_id: ObjectId,
///     text: String
/// }
/// ```
///
/// **RECOMMENDED:**
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
/// #[serde(transparent)]
/// pub struct PostId(pub ObjectId);
///
/// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
/// #[serde(transparent)]
/// pub struct CommentId(pub ObjectId);
///
/// #[derive(Serialize, Deserialize, Entity)]
/// struct Post {
///     id: PostId,
///     text: String,
/// }
///
/// #[derive(Serialize, Deserialize, Entity)]
/// struct Comment {
///     id: CommentId,
///     post_id: PostId,
///     text: String
/// }
/// ```
///
/// ## 2. Isolate database logic in modules
///
/// If your app is even moderately complex, keep each entity in its own module, and avoid using Khan types like
/// `Entity` or `Mongo` directly from controller code.
///
/// A layered architecture helps keep database concerns isolated, making your application easier to reason
/// about, test, and maintain as it grows.
///
/// **NOT RECOMMENDED:**
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Entity)]
/// struct Post {
///     id: ObjectId,
///     text: String,
/// }
///
/// // Controller code
/// async fn create_post(ctx: AppContext, text: String) -> Result<()> {
///     Post {
///         id: ObjectId::new(),
///         text,
///     }
///     .insert(ctx.mongo())
///     .await?;
///
///     Ok(())
/// }
/// ```
///
/// **RECOMMENDED:**
///
/// ```ignore
/// mod model {
///     mod entity {
///         use super::*;
///
///         #[derive(Serialize, Deserialize, Entity)]
///         pub struct PostEntity {
///             id: ObjectId,
///             text: String,
///         }
///     }
///
///     pub struct Post(entity::PostEntity);
///
///     impl Post {
///         pub fn id(&self) -> PostId {
///             self.0.id
///         }
///
///         pub fn text(&self) -> &str {
///             &self.0.text
///         }
///
///         pub async fn create(mongo: impl Mongo, text: String) -> Result<Self> {
///             let entity = entity::PostEntity {
///                 id: ObjectId::new(),
///                 text
///             };
///
///             entity.insert(mongo).await?;
///
///             Ok(Self(entity))
///         }
///     }
/// }
///
/// async fn create_post(ctx: AppContext, text: String) -> Result<()> {
///     model::Post::create(ctx.mongo(), text).await?;
///
///     Ok(())
/// }
/// ```
///
/// In a medium-to-large app, the added boilerplate will quickly pay off. To reduce that boilerplate, consider
/// using crates like [`delegate`](https://crates.io/crates/delegate) or
/// [`accessory`](https://crates.io/crates/accessory) to generate common forwarding logic.
///
/// ## 3. Implement custom methods on entities
///
/// Khan focuses on simple CRUD operations. It intentionally avoids covering more complex cases like:
/// - updates on nested documents or arrays,
/// - advanced query or projection logic,
/// - multi-stage conditional operations.
///
/// Re-implementing these features generically in Rust would require a complex DSL, which would likely be more
/// confusing than helpful.
///
/// Instead, we recommend defining your own type-safe interfaces for advanced operations on a case-by-case
/// basis. A good pattern is to implement them as additional methods on your `Entity` structs:
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Entity)]
/// struct User {
///     #[serde(rename = "_id")]
///     id: ObjectId,
///     name: String,
///     password: String,
///     sessions: Vec<Session>,
/// }
///
/// struct Session {
///     id: ObjectId,
///     ip_addr: IpAddr,
/// }
///
/// impl User {
///     /// Adds a new session to the user.
///     /// This operation uses a raw `$push` update on a nested array field, which is not supported by Khan's
///     /// typed update API — so we use `UntypedUpdateApply`.
///     pub async fn add_session(&mut self, mongo: impl Mongo, session: Session) -> Result<()> {
///         self.patch(
///             mongo,
///             UntypedUpdateApply::new(
///                 // Construct a raw MongoDB update document:
///                 // { "$push": { "sessions": <serialized session> } }
///                 doc! {
///                     "$push": { "sessions": bson::to_bson(&session).unwrap() }
///                 },
///                 // Apply the same mutation to the in-memory struct
///                 |user| {
///                     user.sessions.push(session);
///                 },
///             ),
///         ).await?;
///
///         Ok(())
///     }
/// }
/// ```
///
/// This keeps your API clean and expressive, while giving you full control over how each operation behaves.
pub mod c5_patterns_and_recommendations {}

/// # Indexes and Schema Validation
///
/// `khan` can optionally manage indexes, query validation rules, and JSON Schema validation for your `MongoDB`
/// collections. These features are disabled by default and can be enabled using crate features.
///
/// ## Enabling metadata support
///
/// To enable index and validation rule management:
/// - Use the `meta` feature to enable index and
///   [query validation](https://www.mongodb.com/docs/manual/core/schema-validation/specify-query-expression-rules/)
///   enforcement.
/// - Use the `schema` feature to enable
///   [`MongoDB` JSON Schema validation](https://www.mongodb.com/docs/manual/core/schema-validation/specify-json-schema/)
///   via the [`schemars`] crate.
///
/// ## Defining indexes
///
/// Indexes can be declared on your entities using the `#[entity(indexes(...))]` attribute. This is type-safe
/// — the compiler checks that all referenced fields actually exist.
///
/// ```ignore
/// use khan::Entity;
/// use mongodb::{bson::oid::ObjectId, options::IndexOptions};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Entity)]
/// #[entity(
///     skip_schema_validation,
///     indexes(
///         // the name of the index
///         email_idx(
///             // index keys and their directions
///             keys(email = 1),
///             // optional - additional index options
///             options = IndexOptions::builder()
///                 .sparse(true)
///                 .build()
///         )
///     )
/// )]
/// struct User {
///     #[serde(rename = "_id")]
///     id: ObjectId,
///     email: String,
///     password: String,
/// }
/// ```
///
/// ```
/// # use khan::Entity;
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// let indexes = test_support::IndexedUser::indexes();
/// assert_eq!(indexes.len(), 1);
/// assert_eq!(indexes[0].keys, doc! { "email": 1 });
/// assert_eq!(indexes[0].options.as_ref().unwrap().name.as_deref(), Some("email_idx"));
/// assert_eq!(indexes[0].options.as_ref().unwrap().sparse, Some(true));
/// ```
///
/// Notes:
/// - Use `1` for ascending indexes and `-1` for descending indexes.
/// - If the index name is set to `__`, Khan treats it as an unnamed index, and `MongoDB` will
///   generate the name automatically.
/// - To apply indexes to collections at runtime, call:
///   ```ignore
///   khan::meta::enforce_indexes(mongo).await?;
///   ```
///
/// ## Query validation
///
/// `MongoDB` supports
/// [per-collection validation rules](https://www.mongodb.com/docs/manual/core/schema-validation/specify-query-expression-rules/)
/// that restrict allowed document writes. You can declare
/// query validation rules using the `#[entity(query_validation = ...)]` attribute.
///
/// ```ignore
/// use khan::Entity;
/// use mongodb::bson::{doc, oid::ObjectId};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Entity)]
/// #[entity(
///     skip_schema_validation,
///     query_validation = doc! {
///         "$gt": [ { "$strLenCP": { "$getField": user::Fields::Name } }, 2 ]
///     }
/// )]
/// struct User {
///     #[serde(rename = "_id")]
///     id: ObjectId,
///     name: String,
/// }
/// ```
///
/// ```
/// # use khan::Entity;
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// assert_eq!(
///     test_support::IndexedUser::query_validation(),
///     Some(doc! {
///         "$gt": [
///             { "$strLenCP": { "$getField": test_support::indexed_user::Fields::Name } },
///             2
///         ]
///     })
/// );
/// ```
///
/// Validation rules can be applied by calling:
/// ```ignore
/// khan::meta::enforce_validation(mongo).await?;
/// ```
///
/// ## JSON Schema validation
///
/// If the `schema` feature is enabled, Khan uses the [`schemars`] crate to generate
/// [`MongoDB` JSON Schema validation rules](https://www.mongodb.com/docs/manual/core/schema-validation/specify-json-schema/).
/// You can disable it per-entity using:
/// ```ignore
/// #[entity(skip_schema_validation)]
/// ```
///
/// Entities using schema validation must implement [`schemars::JsonSchema`].
///
/// Validation rules can be applied by calling:
/// ```ignore
/// khan::meta::enforce_validation(mongo).await?;
/// ```
///
/// ## BSON-compatible schema types
///
/// MongoDB’s JSON Schema implementation does not support certain standard keywords, such as the `"integer"`
/// type. To work around this, Khan provides BSON-compatible wrapper types in the [`types`](crate::types) module. Use
/// these as drop-in replacements in any entity that uses JSON Schema validation:
///
/// - `Int32` instead of `i32`
/// - `Int64` instead of `i64`
/// - `ObjectId`
/// - `Regex`
/// - `JavaScriptCode`
/// - `JavaScriptCodeWithScope`
/// - `Timestamp`
/// - `Binary`
/// - `DateTime`
/// - `Decimal128`
///
/// ## Use caution in production
///
/// `enforce_indexes` and `enforce_validation` apply changes directly to your database, and may
/// come in conflict with existing database state (e.g. existing named indexes). They are best suited
/// for development or simple use cases.
///
/// For more advanced setups (e.g. production migrations), use the lower-level API:
///
/// ```ignore
/// for metadata in khan::meta::entity_metadata() {
///     println!("Collection: {}", metadata.collection_name());
///     // Handle custom migration, validation, or indexing logic here
/// }
/// ```
///
/// Each [`EntityMetadata`](crate::meta::EntityMetadata) item includes declared indexes
/// and validation rules for one entity, giving you full control over how they're applied.
pub mod c6_indexes_and_schema_validation {}

/// This library is named "`khan`" after Genghis Khan, because "Mongo" is a prefix to "Mongolia".
pub mod c7_naming {}
