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
///   [`ObjectId`](mongodb::bson::oid::ObjectId).
///
/// The type of the `id` field may be [`ObjectId`](mongodb::bson::oid::ObjectId) itself,
/// or a newtype wrapper around it. See
/// [this note](https://docs.rs/khan/latest/khan/guides/patterns_and_recommendations/index.html#use-newtypes-for-ids)
/// for why using a newtype might be a good idea.
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
/// collection. By default, the collection name is the lowercase form of the struct name
/// (e.g., `User` → `user`). You can override this using the
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
/// # async fn run(mut mongo: khan::Mongo<'static>) -> mongodb::error::Result<()> {
/// let user = User {
///   id: ObjectId::new(),
/// #   email: test_support::unique("kit@example.com"),
/// #   avatar_url: "https://example.com/kit.png".into(),
///   name: "Kit Isaev".into(),
///   password: "somepassword".into(),
/// #   index: 0.into(),
/// };
/// # let user_id = user.id;
///
///
/// // Equivalent to:
/// // db.user.insertOne({ _id: user.id, name: "Kit Isaev", password: "somepassword" })
/// user.insert(mongo.rb()).await?;
///
/// // Equivalent to:
/// // db.user.findOne({ _id: user.id })
/// let user = User::find_one(mongo.rb(), by_id(user_id)).await?.unwrap();
/// assert_eq!(user.id, user_id);
///
/// // Equivalent to:
/// // db.user.deleteOne({ _id: user.id })
/// let result = user.remove(mongo.rb()).await?;
/// assert!(result.deleted());
///
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
/// ## Creating `Mongo`
///
/// [`Mongo`](crate::Mongo) is a lightweight wrapper around a reference to
/// [`mongodb::Database`](mongodb::Database), optionally paired with a mutable reference to
/// a [`mongodb::ClientSession`](mongodb::ClientSession) for use in
/// [transactions](super::transactions_and_locking).
///
/// It is accepted by all Khan operations and can be created from a
/// [`Database`](mongodb::Database) instance:
///
/// ```ignore
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
/// Methods in `khan` take `Mongo` by value. To reuse the same instance multiple times,
/// call [`.rb()`](crate::Mongo::rb) to reborrow it:
///
/// ```
/// # use khan::{Entity, Mongo, Selectable, SelectableWithId};
/// # #[path = "test_support.rs"] mod test_support;
/// # use test_support::{RUNTIME, User, mongo, user};
/// # async fn run(mut setup_mongo: khan::Mongo<'static>) -> mongodb::error::Result<()> {
/// # let db = setup_mongo.db.clone();
/// # let email = test_support::unique("kit@example.com");
/// # let mut seed = test_support::user();
/// # seed.email = email.clone();
/// # seed.insert(setup_mongo.rb()).await?;
/// let mut mongo = Mongo::new(&db);
///
/// let user = User::find_one(mongo.rb(), user::filter! {
///     email: &email
/// }).await?;
///
/// if let Some(user) = user {
///     let result = user.remove(mongo.rb()).await?;
///     assert!(result.deleted());
/// }
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
/// ## Method overview
///
/// | Method name                       | Description                                                                      | Example                                                                                                 | Corresponding MongoDB Query                                                                   |  
/// |-----------------------------------|----------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|  
/// | `Entity::insert`                  | Inserts a new entity into the database.                                          | `User { id, name: "Kit".into(), password: "pass".into() }.insert(mongo).await?;`                        | `db.collection('user').insertOne({ _id: id, name: "Kit", password: "pass" });`                |  
/// | `Entity::insert_many`             | Inserts multiple entities into the database.                                     | `User::insert_many(mongo, &[User { id, name: "Kit".into(), password: "pass".into() }]).await?;`         | `db.collection('user').insertMany([{ _id: id, name: "Kit", password: "pass" }]);`             |
/// | `Entity::count`                   | Counts entities matching a filter.                                               | `User::count(mongo, user::filter! { name: "Kit" }).await?;`                                             | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Entity::exists`                  | Returns true if at least one entity matches the filter.                          | `User::exists(mongo, user::filter! { name: "Kit" }).await?;`                                            | `db.collection('user').count({ name: { $eq: "Kit" } });`                                      |
/// | `Selectable::find`                | Finds entities based on a filter.                                                | `User::find(mongo, user::filter! { name: "Kit" }).await?;`                                              | `db.collection('user').find({ name: { $eq: "Kit" } });`                                       |  
/// | `Selectable::find_one`            | Finds a single entity based on a filter.                                         | `User::find_one(mongo, by_id(id)).await?;`                                                              | `db.collection('user').findOne({ _id: { $eq: id } });`                                        |
/// | `Selectable::find_with_opts`      | Finds entities with options for skip, limit, and sorting.                        | `User::find_with_opts(user::filter! { name: "Kit" }), by_id(id), Some(10), Some(20), None).await?;`     | `db.collection('user').find({ name: { $eq: "Kit" } }).skip(10).limit(20);`                    |  
/// | `Selectable::find_one_and_update` | Finds and updates a single entity based on a filter.                             | `User::find_one_and_update(mongo, by_id(id), user::update! { name: "Kit".into() }).await?;`             | `db.collection('user').findOneAndUpdate({ _id: id }, { $set: { name: "Kit" } });`             |
/// | `Entity::update`                  | Updates multiple documents based on a filter.                                    | `User::update(mongo, user::filter! { name: "Kit" }, user::update! { password: "pass".into() }).await?;` | `db.collection('user').updateMany({ name: { $eq: "Kit" } }, { $set: { password: "pass" } });` |  
/// | `Entity::update_one`              | Updates a single document based on a filter.                                     | `Entity::update_one(mongo, by_id(id), user::update! { password: "pass".into() }).await?;`               | `db.collection('user').updateOne({ _id: { $eq: id } }, { $set: { password: "pass" } });`      |
/// | `SelectableWithId::patch`         | Applies a patch to an existing document based on its id, and updates the struct. | `user.patch(mongo, user::update! { password: "pass".into() }).await?;`                                  | `db.collection('user').updateOne({ _id: { $eq: user.id } }, { $set: { password: "pass" } });` |
/// | `Entity::delete`                  | Deletes multiple documents based on a filter.                                    | `User::delete(mongo, user::filter! { name: "Kit" }).await?;`                                            | `db.collection('user').deleteMany({ name: { $eq: "Kit" } });`                                 |  
/// | `Entity::delete_one`              | Deletes a single document based on a filter.                                     | `Entity::delete_one(mongo, by_id(id)).await?;`                                                          | `db.collection('user').deleteOne({ _id: { $eq: id } });`                                      |  
/// | `SelectableWithId::remove`        | Removes an existing entity from the database by id.                              | `user.remove(mongo).await?;`                                                                            | `db.collection('user').deleteOne({ _id: { $eq: user.id } });`                                 |
mod getting_started {}

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
///     name: Field::Set(FilterOperator::Eq("Kit")),
///     ..Default::default()
/// };
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$eq": "Kit" } });
/// ```
///
/// Equivalent `MongoDB` query:
///
/// ```mongodb
/// db.user.findOne({ name: { $eq: "Kit" } });
/// ```
///
/// ```
/// # use khan::{Field, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::TypedUpdate {
///     name: Field::Set("K.I.".to_string()),
///     ..Default::default()
/// };
///
/// assert_eq!(update.to_document(), doc! { "$set": { "name": "K.I." } });
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
/// # use khan::{Filter, Selectable};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::filter! {
///     name: "Kit"
/// };
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$eq": "Kit" } });
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Filter, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Eq("Kit")),
///     ..Default::default()
/// };
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$eq": "Kit" } });
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
///     name: Ne("Kit")
/// };
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$ne": "Kit" } });
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Filter, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let filter = user::TypedFilter {
///     name: Field::Set(FilterOperator::Ne("Kit")),
///     ..Default::default()
/// };
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$ne": "Kit" } });
/// ```
///
/// And for updates:
/// ```
/// # use khan::{Selectable, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::update! {
///     name: "Kit".to_string()
/// };
///
/// assert_eq!(update.to_document(), doc! { "$set": { "name": "Kit" } });
/// ```
///
/// Expands to:
/// ```
/// # use khan::{Field, Update};
/// # use khan::mongodb::bson::doc;
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::user;
/// let update = user::TypedUpdate {
///     name: Field::Set("Kit".to_string()),
///     ..Default::default()
/// };
///
/// assert_eq!(update.to_document(), doc! { "$set": { "name": "Kit" } });
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
/// let filter: UntypedFilter<()> = UntypedFilter::new(bson::doc! {
///     "name": {
///         "$regex": "^Kit$"
///     }
/// });
///
/// assert_eq!(filter.to_document(), doc! { "name": { "$regex": "^Kit$" } });
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
/// # use test_support::Comment;
/// # let comment = bson::to_bson(&Comment {
/// #     id: ObjectId::new(),
/// #     text: "hi".into(),
/// # }).unwrap();
/// let update: UntypedUpdate<()> = UntypedUpdate::new(bson::doc! {
///     "$push": {
///         "comments": { "$each": [comment.clone()], "$slice": -10 }
///     }
/// });
///
/// assert_eq!(
///     update.to_document(),
///     bson::doc! { "$push": { "comments": { "$each": [comment], "$slice": -10 } } }
/// );
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
///     "name": { "$regex": "^Kit$" }
/// });
///
/// assert_eq!(filter.to_document(), bson::doc! { "name": { "$regex": "^Kit$" } });
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
///     user::Fields::Name: { "$regex": "^Kit$" }
/// });
///
/// assert_eq!(filter.to_document(), bson::doc! { "name": { "$regex": "^Kit$" } });
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
/// # async fn run(mut mongo: khan::Mongo<'static>) -> mongodb::error::Result<()> {
/// # let mut post = test_support::post();
/// # post.insert(mongo.rb()).await?;
/// let result = post.patch(mongo.rb(), UntypedUpdateApply::new(
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
mod filters_and_updates {}

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
/// # async fn run(mut mongo: khan::Mongo<'static>) -> mongodb::error::Result<()> {
/// # let name = test_support::unique("Kit");
/// # let mut seed = test_support::user();
/// # seed.name = name.clone();
/// # seed.insert(mongo.rb()).await?;
/// let mut profile = user::PublicProfile::find_one(mongo.rb(), user::filter! {
///     name: &name
/// }).await?.unwrap();
///
/// let result = profile.patch(mongo.rb(), user::update! { name: "Tom".into() }).await?;
/// assert!(result.matched());
/// assert_eq!(&profile.name, "Tom");
/// # Ok(())
/// # }
/// # RUNTIME.block_on(run(mongo())).unwrap();
/// ```
///
mod projections {}

/// # Transactions and locking
///
/// All methods on [`Entity`](crate::Entity), [`Selectable`](crate::Selectable), and
/// [`SelectableWithId`](crate::SelectableWithId) can be run in the context of a
/// transaction. To do this, start a transaction using the regular
/// [`mongodb` crate API](mongodb::ClientSession), then construct a [`Mongo`](crate::Mongo)
/// instance using `(&Database, &mut ClientSession)` instead of just `&Database`:
///
/// ```ignore
/// let client = Client::with_uri_str("mongodb://localhost:27017").await?;
/// let db = client.database("mydb");
///
/// let mut session = client.start_session().await?;
///
/// session.start_transaction().and_run(&db, |session| async move {
///     let mut mongo = (db, session).into();
///
///     let user = User::find_one(mongo.rb(), user::filter! {
///         email: "kit@example.com"
///     }).await?;
///
///     if let Some(user) = user {
///         user.remove(mongo.rb()).await?;
///     }
///
///     Ok(())
/// }).await?;
/// ```
///
/// ## Locking
///
/// Sometimes you want to make sure that a document read inside a transaction
/// isn’t modified by another operation before the transaction commits.
///
/// For example, imagine you're inserting a `Comment` that references an existing `Post`
/// by its ID. You check that the referenced post exists in the beginning of the
/// transaction, and want to make sure that it is not deleted before the transaction
/// commits:
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session).into();
///
///             if !Post::exists(mongo.rb(), by_id(post_id)).await? {
///                 return Err(Error::custom("Post is not found"));
///             }
///
///             // Post may be deleted betweeen these two operations,
///             // rendering a reference by id invalid.
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(mongo.rb())
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
/// are needed — the update itself will act as a lock.
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session).into();
///
///             // This update acts as a lock by modifying the document
///             let result = Post::update_one(
///                 mongo.rb(),
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
///             // Safe to insert the comment now — if the post were deleted concurrently,
///             // the transaction would fail due to a write conflict on the post.
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(mongo.rb())
///             .await?;
///
///             Ok(())
///         },
///     )
///     .await?;
/// ```
///
/// However, if no meaningful changes are required, you can perform a *dummy update*
/// by writing to an unused utility field, such as `_lock.seed`, with a random value:
///
/// ```ignore
/// session
///     .start_transaction()
///     .and_run(
///         (&db, post_id, text),
///         |session, (db, post_id, text)| async move {
///             let mut mongo = (db, session).into();
///
///             // We're not making any meaningful changes to the Post,
///             // but we still want to ensure it won't be modified or deleted during the transaction.
///             Post::update_one(
///                 mongo.rb(),
///                 by_id(post_id),
///                 UntypedUpdate::new(doc! {
///                     "$set": { "_lock": { "seed": ObjectId::new() } }
///                 }),
///             )
///             .await?;
///
///             Comment {
///                 id: ObjectId::new(),
///                 post_id,
///                 text,
///             }
///             .insert(mongo.rb())
///             .await?;
///
///             Ok(())
///         },
///     )
///     .await?;
/// ```
///
/// This locking technique works well when the entire transaction happens within a
/// single method or scope.
///
/// However, if a transaction spans multiple methods, it can become difficult to track which
/// documents have been locked and which haven’t. This makes it easy to accidentally skip a
/// necessary lock, leading to race conditions or inconsistent state:
///
/// ```ignore
/// // Model code
/// async fn create_post(trx: Transaction<'_>, text: String) -> Result<ObjectId> {
///     let id = ObjectId::new();
///
///     // Insert a new post document into the database.
///     Post {
///         id,
///         text,
///     }
///     .insert(trx.into())
///     .await?;
///
///     Ok(id)
/// }
///
/// // Model code
/// async fn create_comment(trx: Transaction<'_>, post_id: ObjectId, text: String) -> Result<()> {
///     // Insert a new comment referencing the given post_id.
///     Comment {
///         id: ObjectId::new(),
///         post_id,
///         text,
///     }
///     .insert(trx.into())
///     .await?;
///
///     Ok(())
/// }
///
/// // Controller code
/// async fn make_post_with_initial_comment(ctx: AppContext, post_text: String, comment_text: String) -> Result<()> {
///     ctx.mongo().run_transaction((post_text, comment_text), |trx, (post_text, comment_text)| async move {
///         // The post is created as part of this transaction...
///         let post_id = create_post(trx.rb().into(), post_text).await?;
///
///         // ...and the comment referencing it is inserted within the same transaction.
///         // This is safe: the post is guaranteed to not be deleted or modified until the transaction commits.
///         create_comment(trx.rb().into(), post_id, comment_text).await?;
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
///         // This is NOT safe: we assume the post exists,
///         // but there's no guarantee it won't be deleted before the transaction commits.
///         // This can result in a comment pointing to a non-existent post.
///         create_comment(trx.into(), post_id, text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
/// ```
///
/// In these cases, it may be desirable encode the locking guarantee in the type system.
///
/// Khan provides a [`Lock<T>`](crate::Lock) wrapper type to express this guarantee explicitly in your method
/// signatures. When a value is wrapped in [`Lock<T>`](crate::Lock), it means that the document has already been
/// locked (via a dummy or real update), and it will not be modified again until the transaction completes.
///
/// You can then require a [`Lock<T>`](crate::Lock) as input to any method that assumes the document is
/// protected from concurrent modification:
///
/// ```
/// # use khan::{Entity, Lock};
/// # #[path = "test_entities.rs"] mod test_support;
/// # use test_support::post;
/// let locked_post = Lock::new_unchecked(post());
/// let locked_id = locked_post.locked_id();
///
/// assert_eq!(*locked_id, locked_post.id);
/// ```
///
/// ```ignore
/// // Model code
/// async fn create_post(
///     trx: Transaction<'_>,
///     text: String,
/// ) -> Result<Lock<ObjectId>> {
///     // This function guarantees at the type level that the post it creates
///     // will not be modified or deleted until the transaction is committed.
///
///     let id = ObjectId::new();
///
///     // Insert the post while marking it as "locked" within the transaction.
///     let post: Lock<Post> = Post {
///         id,
///         text,
///     }
///     .locking_insert(trx.into())
///     .await?;
///
///     // Convert the locked post into a locked ID, so we can safely pass it to other methods.
///     let locked_id: Lock<ObjectId> = post.locked_id();
///
///     Ok(locked_id)
/// }
///
/// // Model code
/// async fn create_comment(
///     trx: Transaction<'_>,
///     post_id: Lock<ObjectId>, // Enforces that the referenced post is already locked
///     text: String,
/// ) -> Result<()> {
///     // This function requires, at the type level, that the referenced post is locked.
///     // This ensures the post can't be modified or deleted during the transaction.
///
///     Comment {
///         id: ObjectId::new(),
///         post_id,
///         text,
///     }
///     .insert(trx.into())
///     .await?;
///
///     Ok(())
/// }
///
/// // Model code
/// async fn reference_post(trx: Transaction<'_>, post_id: ObjectId) -> Result<Lock<ObjectId>> {
///     // Attempts to find and lock the post by ID, returning a locked ID if successful.
///     // If the post does not exist, returns an error.
///     match Post::lock_by_id(trx, post_id).await? {
///         Some(locked_id) => Ok(locked_id),
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
///         // Creates a new post - it is locked since it has just been inserted.
///         let post_id: Lock<ObjectId> = create_post(trx.rb().into(), post_text).await?;
///
///         // Since the post is locked, we can safely insert a comment referencing it
///         create_comment(trx.rb().into(), post_id, comment_text).await?;
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
///         // Ensure that the post exists and is locked before proceeding
///         let post_id: Lock<ObjectId> = reference_post(trx.rb().into(), post_id).await?;
///
///         // Now that the locking guarantee is enforced by the type system,
///         // `create_comment` cannot be called unless the post is locked.
///         create_comment(trx.into(), post_id, text).await?;
///
///         Ok(())
///     }).await?;
///
///     Ok(())
/// }
/// ```
mod transactions_and_locking {}

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
/// Instead of using `ObjectId` directly in your entities, define a newtype wrapper:
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
/// #[serde(transparent)]
/// pub struct PostId(pub ObjectId);
/// ```
///
/// This helps avoid mixing up IDs of different entities and improves type safety across your codebase.
///
/// You can still use this newtype as an `Entity::Id` type, and Khan will handle it like a regular `ObjectId`.
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
///         pub async fn create(mongo: Mongo<'_>, text: String) -> Result<Self> {
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
/// ## 3. Implement custom methods on entitiesx
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
///     pub async fn add_session(&mut self, mongo: Mongo<'_>, session: Session) -> Result<()> {
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
mod patterns_and_recommendations {}

/// # Indexes and Schema Validation
///
/// `khan` can optionally manage indexes, query validation rules, and JSON Schema validation for your `MongoDB`
/// collections. These features are disabled by default and can be enabled using crate features.
///
/// ## Enabling metadata support
///
/// To enable index and validation rule management:
/// - Use the `meta` feature to enable index and query validation enforcement.
/// - Use the `schema` feature to enable JSON Schema validation via the `schemars` crate.
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
/// - Use quoted strings for descending indexes: `keys(created_at = "-1")`.
/// - If the index name is set to `__`, `MongoDB` will generate the name automatically.
/// - To apply indexes to collections at runtime, call:
///   ```ignore
///   khan::meta::enforce_indexes(mongo).await?;
///   ```
///
/// ## Query validation
///
/// `MongoDB` supports per-collection validation rules that restrict allowed query shapes. You can declare
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
/// If the `schema` feature is enabled, Khan will generate JSON Schema validation rules for all entities by
/// default. You can disable it per-entity using:
/// ```ignore
/// #[entity(skip_schema_validation)]
/// ```
///
/// Entities using schema validation must implement `schemars::JsonSchema`.
///
/// ## BSON-compatible schema types
///
/// MongoDB’s JSON Schema implementation does not support certain standard keywords, such as the `"integer"`
/// type. To work around this, Khan provides BSON-compatible wrapper types in the `khan::types` module. Use
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
/// `enforce_indexes`, `enforce_validation`, and `enforce_schema` apply changes directly to your database,
/// and may come in conflict with existing database state (e.g. existing named indexes). They are best suited
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
/// Each `EntityMetadata` item includes declared indexes and validation rules for one entity, giving you full
/// control over how they're applied.
mod indexes_and_schema_validation {}

/// This library is named "`khan`" because "Mongo" is a prefix to "Mongolia".
mod naming {}
