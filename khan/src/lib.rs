//! Khan is a `MongoDB` ODM for Rust.
//!
//! ## Example
//!
//! ```ignore
//! # use khan::{Entity, Selectable, SelectableWithId, by_id};
//! # #[path = "test_support.rs"] mod test_support;
//! # use test_support::{RUNTIME, mongo};
//! # use mongodb::bson::oid::ObjectId;
//! # use serde::{Deserialize, Serialize};
//! # async fn run() -> mongodb::error::Result<()> {
//! # let mongo = mongo();
//! // Define an entity
//! #[derive(Serialize, Deserialize, Entity, Debug, PartialEq, Eq)]
//! #[entity(skip_schema_validation, collection = "readme_user", projections(Profile(id, email, password)))]
//! struct User {
//!   #[serde(rename = "_id")]
//!   id: ObjectId,
//!   email: String,
//!   username: String,
//!   password: String,
//! }
//!
//! // Insert an entity into the database
//! let user = User {
//!   id: ObjectId::new(),
//!   email: "mail@example.com".into(),
//!   username: "nikis05".into(),
//!   password: "somepassword".into(),
//! };
//! let user_id = user.id;
//!
//! user.insert(mongo).await?;
//!
//! // Select an entity by id
//! let person: User = User::find_one(mongo, by_id(user_id)).await?.unwrap();
//! # assert_eq!(person.email, "mail@example.com");
//!
//! // Select an entity by custom fields
//! let recent_user: User = User::find_one(mongo, user::filter! {
//!   username: "nikis05"
//! }).await?.unwrap();
//! # assert_eq!(recent_user.id, user_id);
//!
//! // Select only necessary fields (email, password) of entity
//! let profile: user::Profile = user::Profile::find_one(mongo, by_id(user_id)).await?.unwrap();
//! # assert_eq!(profile.password, "somepassword");
//!
//! // Update an entity in the database
//! User::update_one(mongo, by_id(user_id), user::update! {
//!   email: "new.email@example.com".into()
//! }).await?;
//! # assert_eq!(User::find_one(mongo, by_id(user_id)).await?.unwrap().email, "new.email@example.com");
//!
//! // Update an entity in the database (struct is automatically updated)
//! let mut user = User::find_one(mongo, by_id(user_id)).await?.unwrap();
//! user.patch(mongo, user::update! {
//!   email: "newer.email@example.com".into(),
//!   password: "someotherpassword".into()
//! }).await?;
//! # assert_eq!(user.password, "someotherpassword");
//!
//! // Delete entities matching the filter
//! let result = User::delete_one(mongo, by_id(user_id)).await?;
//! # assert!(result.deleted());
//!
//! // Remove a document from the database that corresponds to an instance
//! let removable = User {
//!   id: ObjectId::new(),
//!   email: "remove@example.com".into(),
//!   username: "remove-me".into(),
//!   password: "temporary".into(),
//! };
//! let removable_id = removable.id;
//! removable.insert(mongo).await?;
//! removable.remove(mongo).await?;
//! # assert!(User::find_one(mongo, by_id(removable_id)).await?.is_none());
//! # Ok(())
//! # }
//! # RUNTIME.block_on(run()).unwrap();
//! ```
//!
//! See [`guides`] module to learn more!

#![warn(clippy::pedantic, missing_docs)]
#![allow(
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::missing_errors_doc
)]

use futures_util::{
    FutureExt, TryStreamExt,
    future::{BoxFuture, LocalBoxFuture},
};
use mongodb::{
    ClientSession, Collection, Database,
    bson::{self, Bson, Document, bson, doc},
    error::Result,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt::Display, hash::Hash, marker::PhantomData, sync::LazyLock};

pub use indexmap::{self, IndexMap};
#[doc(hidden)]
pub use khan_macros::{__private__construct_filter, __private__construct_update};
pub use khan_macros::{Entity, Fields};
pub use mongodb;

#[cfg(any(doctest, test))]
extern crate self as khan;

/// High-level usage guides for Khan, covering core concepts like CRUD, filters, projections, transactions,
/// and design patterns. Start here.
pub mod guides;
/// Tools for managing indexes and validation rules on collections.
///
/// Enabled with the `meta` feature.
#[cfg(feature = "meta")]
pub mod meta;
#[cfg(any(doctest, test))]
#[doc(hidden)]
pub mod test_support;
/// BSON-compatible types for use with JSON Schema validation.
///
/// These types serve as drop-in replacements for BSON types that are not  supported by `MongoDB`'s JSON Schema
/// implementation.
///
/// Enabled with the `schema` feature.
#[cfg(feature = "schema")]
pub mod types;

/// Core trait representing a `MongoDB` document.
///
/// Types that implement `Entity` can be inserted, updated, deleted, and queried by ID. Each entity maps to a
/// single `MongoDB` collection.
///
/// This trait should not be implemented manually — use `#[derive(Entity)]` instead.
///
/// Types that derive `Entity`:
/// - must also derive [`Serialize`] and [`Deserialize`](serde::Deserialize),
/// - must have a field named `id` with `#[serde(rename = "_id")]`,
/// - the type of the `id` field must implement [`Copy`], and be serializable /
///   deserializable to / from [`ObjectId`](mongodb::bson::oid::ObjectId).
pub trait Entity: SelectableWithId<Self> + Serialize {
    /// Type of the entity’s primary key (`id` field).
    ///
    /// Typically this is `ObjectId`, or a newtype wrapper around it.
    /// The type must serialize and deserialize compatibly with `MongoDB`'s `_id` field.
    type Id: Clone + Serialize + Send + 'static;

    /// Enum representing the entity’s field names, used for sorting and indexing.
    ///
    /// This type is automatically generated when you derive `Entity`, and includes
    /// all fields in the struct.
    ///
    /// You can access it as `helper_module::Fields`, where `helper_module` is the lowercase name of the
    /// entity (e.g. `user::Fields` for `User`).
    type Fields: Display + Send + Eq + Hash + 'static;

    /// Name of the `MongoDB` collection this entity is stored in.
    ///
    /// By default, this is the lowercase name of the struct (e.g. `"user"` for `User`). If the entity name
    /// ends with an `Entity` suffix, e.g. `UserEntity`, that suffix is stripped.
    ///
    /// You can override it using the `#[entity(collection = "...")]` attribute.
    const COLLECTION_NAME: &'static str;

    /// Returns a handle to the underlying `MongoDB` collection for this entity.
    fn collection(db: &Database) -> Collection<Self> {
        db.collection(Self::COLLECTION_NAME)
    }

    /// Returns the list of indexes defined for this entity.
    ///
    /// This is used by `khan::meta::enforce_indexes` to apply index definitions to the `MongoDB` collection
    /// at runtime.
    ///
    /// When deriving [`Entity`], the implementation is configured by the `#[entity(indexes(...))]` attribute.
    #[cfg(feature = "meta")]
    fn indexes() -> Vec<mongodb::IndexModel> {
        vec![]
    }

    /// Returns the query validation rule defined for this entity, if any.
    ///
    /// This is used by `khan::meta::enforce_validation` to apply query constraints to the `MongoDB` collection.
    ///
    /// When deriving [`Entity`], the implementation is configured by the `#[entity(query_validation = ...)]`
    /// attribute.
    #[cfg(feature = "meta")]
    fn query_validation() -> Option<Document> {
        None
    }

    /// Counts the number of documents in the collection matching the given filter.
    fn count<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
    ) -> BoxFuture<'a, Result<u64>> {
        async move {
            let collection = Self::collection(mongo.db());
            let filter = filter.to_document()?;

            let count = with_session!(collection.count_documents(filter), mongo.session()).await?;

            Ok(count)
        }
        .boxed()
    }

    /// Checks whether any documents match the given filter.
    fn exists<'a>(
        mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
    ) -> BoxFuture<'a, Result<bool>> {
        async move {
            let count = Self::count(mongo, filter).await?;

            Ok(count > 0)
        }
        .boxed()
    }

    /// Inserts this entity into the corresponding `MongoDB` collection.
    fn insert<'a>(&'a self, mut mongo: impl Mongo + 'a) -> BoxFuture<'a, Result<()>> {
        async move {
            let collection = Self::collection(mongo.db());

            with_session!(collection.insert_one(self), mongo.session()).await?;

            Ok(())
        }
        .boxed()
    }

    /// Like [`insert`](Entity::insert), but returns a [`Fence<Self>`] to indicate the document is protected
    /// by this transaction.
    ///
    /// Because the document is newly inserted within the same transaction, it is not visible to other
    /// operations until the transaction commits.
    ///
    /// The returned [`Fence`] acts as a type-level marker that later code can require before creating
    /// dependent records.
    fn insert_and_fence<'a>(self, trx: impl Transaction + 'a) -> BoxFuture<'a, Result<Fence<Self>>>
    where
        Self: 'a,
    {
        async move {
            Self::insert(&self, trx).await?;

            Ok(Fence(self))
        }
        .boxed()
    }

    /// Inserts multiple entities into the collection in a single batch operation.
    fn insert_many<'a>(
        mut mongo: impl Mongo + 'a,
        entities: &'a [Self],
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            if entities.is_empty() {
                return Ok(());
            }

            let collection = Self::collection(mongo.db());

            with_session!(collection.insert_many(entities), mongo.session()).await?;

            Ok(())
        }
        .boxed()
    }

    /// Like [`insert_many`](Entity::insert_many), but returns a `Vec<Fence<Self>>` to indicate that all
    /// inserted documents are protected by this transaction.
    ///
    /// Because the documents are newly inserted within the same transaction, they are not visible to other
    /// operations until the transaction commits.
    ///
    /// The returned [`Fence`]s act as type-level markers that later code can require before creating
    /// dependent records.
    fn insert_many_and_fence<'a>(
        trx: impl Transaction + 'a,
        entities: Vec<Self>,
    ) -> BoxFuture<'a, Result<Vec<Fence<Self>>>>
    where
        Self: 'a,
    {
        async move {
            Self::insert_many(trx, &entities).await?;

            Ok(entities.into_iter().map(Fence).collect())
        }
        .boxed()
    }

    /// Updates all documents matching the given filter using the provided update.
    fn update<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<UpdateResult>> {
        async move {
            let collection = Self::collection(mongo.db());
            let filter = filter.to_document()?;
            let update = update.to_document()?;

            let result =
                with_session!(collection.update_many(filter, update), mongo.session()).await?;

            Ok(UpdateResult(result))
        }
        .boxed()
    }

    /// Updates a single document matching the given filter using the provided update.
    fn update_one<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<UpdateResult>> {
        async move {
            let collection = Self::collection(mongo.db());
            let filter = filter.to_document()?;
            let update = update.to_document()?;

            let result =
                with_session!(collection.update_one(filter, update), mongo.session()).await?;

            Ok(UpdateResult(result))
        }
        .boxed()
    }

    /// Updates a document by ID and returns a [`Fence<Self::Id>`] for use by later operations in the same
    /// transaction.
    ///
    /// In addition to applying the provided update, this method increments Khan's internal `__fence` field to
    /// force a write conflict if another transaction concurrently modifies or deletes the same document.
    ///
    /// If no document with the given ID exists, returns `None`.
    fn update_by_id_and_fence<'a>(
        mut trx: impl Transaction + 'a,
        id: Self::Id,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<Option<Fence<Self::Id>>>> {
        async move {
            let result = Self::update_one(
                &mut trx,
                by_id(id.clone()),
                merge_fence_into_update(&update)?,
            )
            .await?;

            Ok(if result.matched() {
                Some(Fence(id))
            } else {
                None
            })
        }
        .boxed()
    }

    /// Fences a document by ID for use by later operations in the same transaction, without applying any
    /// domain-level update.
    ///
    /// This method increments Khan's internal `__fence` field to trigger `MongoDB` write conflict detection
    /// if another transaction concurrently modifies or deletes the same document.
    ///
    /// If no document with the given ID exists, returns `None`.
    fn fence_by_id<'a>(
        trx: impl Transaction + 'a,
        id: Self::Id,
    ) -> BoxFuture<'a, Result<Option<Fence<Self::Id>>>> {
        async move {
            let result = Self::update_one(
                trx,
                by_id(id.clone()),
                UntypedUpdate::new(doc! { "$inc": { "__fence": 1 } }),
            )
            .await?;

            Ok(if result.matched() {
                Some(Fence(id))
            } else {
                None
            })
        }
        .boxed()
    }

    /// Deletes all documents matching the given filter.
    fn delete<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
    ) -> BoxFuture<'a, Result<DeleteResult>> {
        async move {
            let collection = Self::collection(mongo.db());
            let filter = filter.to_document()?;

            let result = with_session!(collection.delete_many(filter), mongo.session()).await?;

            Ok(DeleteResult(result))
        }
        .boxed()
    }

    /// Deletes the first document that matches the given filter.
    fn delete_one<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<Self> + 'a,
    ) -> BoxFuture<'a, Result<DeleteResult>> {
        async move {
            let collection = Self::collection(mongo.db());
            let filter = filter.to_document()?;

            let result = with_session!(collection.delete_one(filter), mongo.session()).await?;

            Ok(DeleteResult(result))
        }
        .boxed()
    }
}

/// Trait for types that represent a partial view (projection) of an entity.
///
/// A `Selectable` defines which fields to include when querying documents from `MongoDB`. It is implemented
/// automatically for projection structs declared via the `#[entity(projections(...))]` attribute, and for the
/// entity itself (as the full projection).
///
/// This trait is automatically derived — manual implementation is not required.
pub trait Selectable<E: Entity>: DeserializeOwned + Send + Sync + 'static {
    /// List of fields included in this projection, or `None` if it represents the full entity.
    const FIELDS: Option<&'static [&'static str]>;

    /// Returns a `MongoDB` projection document specifying which fields to include, or `None` if the projection
    /// represents the full entity.
    fn projection() -> Option<Document> {
        static DOCUMENTS: LazyLock<dashmap::DashMap<&'static [&'static str], Document>> =
            LazyLock::new(dashmap::DashMap::new);

        Self::FIELDS.map(|fields| {
            if let Some(document) = DOCUMENTS.get(fields) {
                document.clone()
            } else {
                let mut has_id = false;
                let mut document = doc! {};

                for field in fields {
                    if *field == "_id" {
                        has_id = true;
                    } else {
                        document.insert(*field, 1);
                    }
                }

                if !has_id {
                    document.insert("_id", 0);
                }

                DOCUMENTS.insert(fields, document.clone());
                document
            }
        })
    }

    /// Finds documents matching the given filter, returning this projection type, with optional pagination
    /// and sorting configured by [`FindOptions`].
    fn find_with_opts<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<E> + 'a,
        opts: FindOptions<E>,
    ) -> BoxFuture<'a, Result<Vec<Self>>> {
        async move {
            let collection = mongo.db().collection(E::COLLECTION_NAME);
            let filter = filter.to_document()?;

            let mut query = collection.find(filter);

            if let Some(projection) = Self::projection() {
                query = query.projection(projection);
            }

            if let Some(skip) = opts.skip {
                query = query.skip(skip);
            }

            if let Some(limit) = opts.limit {
                query = query.limit(limit);
            }

            if let Some(sort) = opts.sort {
                let sort_doc = sort
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k.to_string(),
                            match v {
                                Order::Asc => bson!(1),
                                Order::Desc => bson!(-1),
                            },
                        )
                    })
                    .collect();
                query = query.sort(sort_doc);
            }

            let entities = match mongo.session() {
                Some(session) => {
                    query
                        .session(&mut *session)
                        .await?
                        .stream(&mut *session)
                        .try_collect()
                        .await
                }
                None => query.await?.try_collect().await,
            }?;

            Ok(entities)
        }
        .boxed()
    }

    /// Finds all documents matching the given filter and returns them as a list of this projection type.
    fn find<'a>(
        mongo: impl Mongo + 'a,
        filter: impl Filter<E> + 'a,
    ) -> BoxFuture<'a, Result<Vec<Self>>> {
        Self::find_with_opts(mongo, filter, FindOptions::new())
    }

    /// Finds the first document matching the given filter and returns it  as this projection type, or `None`
    /// if no document matches.
    fn find_one<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Self>>> {
        async move {
            let collection = mongo.db().collection(E::COLLECTION_NAME);
            let filter = filter.to_document()?;

            let mut query = collection.find_one(filter);
            if let Some(projection) = Self::projection() {
                query = query.projection(projection);
            }

            let entity = with_session!(query, mongo.session()).await?;

            Ok(entity)
        }
        .boxed()
    }

    /// Finds a single document matching the given filter and returns it wrapped in a [`Fence`].
    ///
    /// This method increments Khan's internal `__fence` field to force a write conflict if another
    /// transaction concurrently modifies or deletes the same document.
    ///
    /// Returns `None` if no documents are found.
    fn find_one_and_fence<'a>(
        trx: impl Transaction + 'a,
        filter: impl Filter<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Fence<Self>>>> {
        async move {
            let entity = Self::find_one_and_update(
                trx,
                filter,
                UntypedUpdate::new(doc! { "$inc": { "__fence": 1 } }),
            )
            .await?;

            Ok(entity.map(Fence))
        }
        .boxed()
    }

    /// Finds a single document matching the given filter, applies the update, and returns the matched document
    /// using this projection type.
    ///
    /// This follows `MongoDB`'s default `findOneAndUpdate` semantics: the returned document is the document
    /// as it existed before the update was applied. Use [`Selectable::find_one`] after this call if you need
    /// to read the updated document.
    ///
    /// Returns `None` if no document matches the filter.
    fn find_one_and_update<'a>(
        mut mongo: impl Mongo + 'a,
        filter: impl Filter<E> + 'a,
        update: impl Update<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Self>>> {
        async move {
            let collection = mongo.db().collection(E::COLLECTION_NAME);
            let filter = filter.to_document()?;
            let update = update.to_document()?;

            let mut query = collection.find_one_and_update(filter, update);

            if let Some(projection) = Self::projection() {
                query = query.projection(projection);
            }

            let entity = with_session!(query, mongo.session()).await?;

            Ok(entity)
        }
        .boxed()
    }

    /// Finds a single document matching the filter, applies the update, and returns the matched document
    /// wrapped in a [`Fence<Self>`].
    ///
    /// This follows `MongoDB`'s default `findOneAndUpdate` semantics: the fenced value is the document as it
    /// existed before the update was applied.
    ///
    /// In addition to the given update, this method increments Khan's internal `__fence` field to force a
    /// write conflict if another transaction concurrently modifies or deletes the same document.
    ///
    /// Returns `None` if no document matches the filter.
    fn find_one_and_update_and_fence<'a>(
        trx: impl Transaction + 'a,
        filter: impl Filter<E> + 'a,
        update: impl Update<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Fence<Self>>>> {
        async move {
            let entity =
                Self::find_one_and_update(trx, filter, merge_fence_into_update(&update)?).await?;

            Ok(entity.map(Fence))
        }
        .boxed()
    }
}

/// Options for [`Selectable::find_with_opts`].
#[derive(Debug)]
pub struct FindOptions<E: Entity> {
    skip: Option<u64>,
    limit: Option<i64>,
    sort: Option<IndexMap<E::Fields, Order>>,
}

impl<E: Entity> FindOptions<E> {
    /// Creates empty find options.
    pub fn new() -> Self {
        Self {
            skip: None,
            limit: None,
            sort: None,
        }
    }

    /// Sets the number of matching documents to skip.
    pub fn skip(mut self, skip: u64) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Sets the maximum number of documents to return.
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the full typed sort specification.
    pub fn sort(mut self, sort: IndexMap<E::Fields, Order>) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Appends or replaces one field in the typed sort specification.
    pub fn sort_by(mut self, field: E::Fields, order: Order) -> Self {
        self.sort
            .get_or_insert_with(IndexMap::new)
            .insert(field, order);
        self
    }
}

impl<E: Entity> Default for FindOptions<E> {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension of [`Selectable`] for projections that include the `id` field.
///
/// Projections implementing this trait can be updated and deleted using convenience methods like
/// [`patch`](SelectableWithId::patch) and [`remove`](SelectableWithId::remove).
///
/// This trait is automatically implemented for projections declared with `id`  as one of their fields, as well
/// as for the entity itself.
pub trait SelectableWithId<E: Entity>: Selectable<E> {
    /// Returns the value of the document’s `id` field.
    fn id(&self) -> E::Id;

    /// Updates the corresponding document in the database and applies the same changes to this in-memory
    /// struct. Note that the struct will be updated even if the corresponding document wasn't updated
    /// (e.g. because it has already been deleted from the database).
    fn patch<'a>(
        &'a mut self,
        mongo: impl Mongo + 'a,
        update: impl Update<E> + UpdateApply<Self> + 'a,
    ) -> BoxFuture<'a, Result<UpdateResult>> {
        async move {
            let update_document = update.to_document()?;
            let result =
                E::update_one(mongo, by_id(self.id()), UntypedUpdate::new(update_document)).await?;

            update.apply(self)?;

            Ok(result)
        }
        .boxed()
    }

    /// Updates the document in the database and the local struct, returning it wrapped in a [`Fence`].
    ///
    /// In addition to applying the provided update, this method increments Khan's internal `__fence` field to
    /// force a write conflict if another transaction concurrently modifies or deletes the same document.
    ///
    /// Returns `Some(Fence<Self>)` if the document was updated, or `None` if no matching document was found.
    fn patch_and_fence<'a>(
        mut self,
        trx: impl Transaction + 'a,
        update: impl Update<E> + UpdateApply<Self> + 'a,
    ) -> BoxFuture<'a, Result<Option<Fence<Self>>>> {
        async move {
            let update_document = update.to_document()?;
            let result =
                E::update_by_id_and_fence(trx, self.id(), UntypedUpdate::new(update_document))
                    .await?;

            if result.is_none() {
                return Ok(None);
            }

            update.apply(&mut self)?;

            Ok(Some(Fence(self)))
        }
        .boxed()
    }

    /// Deletes the corresponding document from the database by its `id`.
    fn remove<'a>(&'a self, mongo: impl Mongo + 'a) -> BoxFuture<'a, Result<DeleteResult>> {
        E::delete_one(mongo, by_id(self.id()))
    }
}

/// Extension methods for [`Database`] that integrate it with Khan.
pub trait DatabaseExt {
    /// Runs a sequence of operations inside a `MongoDB` transaction.
    ///
    /// This method accepts a context value that is passed to the callback on every transaction attempt.
    /// This mirrors [`mongodb::action::StartTransaction::and_run`] and is useful when the callback needs to
    /// borrow data across retries.
    ///
    /// The callback receives a `(&Database, &mut ClientSession)` transaction context and a mutable reference
    /// to the user-provided context.
    ///
    /// ### Example
    ///
    /// ```
    /// # use futures_util::FutureExt;
    /// # use khan::{DatabaseExt, Mongo};
    /// # #[path = "test_support.rs"] mod test_support;
    /// # use test_support::{RUNTIME, mongo};
    /// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
    /// let output = mongo.run_transaction(("text", 42), |trx, (text, number)| async move {
    ///     assert!(trx.db().name().starts_with("khan_test_"));
    ///     Ok(text.len() + *number as usize)
    /// }.boxed()).await?;
    ///
    /// assert_eq!(output, 46);
    /// # Ok(())
    /// # }
    /// # RUNTIME.block_on(run(mongo())).unwrap();
    /// ```
    fn run_transaction<'a, R, C, F>(&'a self, context: C, callback: F) -> BoxFuture<'a, Result<R>>
    where
        R: Send + 'a,
        C: Send + 'a,
        F: for<'b> FnMut(
                (&'b Database, &'b mut ClientSession),
                &'b mut C,
            ) -> BoxFuture<'b, Result<R>>
            + Send
            + 'a;

    /// Runs a sequence of operations inside a `MongoDB` transaction.
    ///
    /// This method is similar to [`DatabaseExt::run_transaction`], but does not take a separate context
    /// value. It mirrors [`mongodb::action::StartTransaction::and_run2`].
    ///
    /// ### Example
    ///
    /// ```
    /// # use khan::{DatabaseExt, Mongo};
    /// # #[path = "test_support.rs"] mod test_support;
    /// # use test_support::{RUNTIME, mongo};
    /// # async fn run(mongo: &'static mongodb::Database) -> mongodb::error::Result<()> {
    /// let output = mongo.run_transaction2(async |trx| {
    ///     assert!(trx.db().name().starts_with("khan_test_"));
    ///     Ok(46)
    /// }).await?;
    ///
    /// assert_eq!(output, 46);
    /// # Ok(())
    /// # }
    /// # RUNTIME.block_on(run(mongo())).unwrap();
    /// ```
    fn run_transaction2<'a, R, F>(&'a self, callback: F) -> LocalBoxFuture<'a, Result<R>>
    where
        R: 'a,
        F: for<'b> AsyncFnMut((&'b Database, &'b mut ClientSession)) -> Result<R> + Send + 'a;
}

impl DatabaseExt for Database {
    fn run_transaction<'a, R, C, F>(&'a self, context: C, callback: F) -> BoxFuture<'a, Result<R>>
    where
        R: Send + 'a,
        C: Send + 'a,
        F: for<'b> FnMut(
                (&'b Database, &'b mut ClientSession),
                &'b mut C,
            ) -> BoxFuture<'b, Result<R>>
            + Send
            + 'a,
    {
        let db = self.clone();

        async move {
            let mut session = db.client().start_session().await?;
            session
                .start_transaction()
                .and_run(
                    (db, context, callback),
                    |session, (db, context, callback)| {
                        async move {
                            let output = callback((db, session), context).await?;
                            Ok(output)
                        }
                        .boxed()
                    },
                )
                .await
        }
        .boxed()
    }

    fn run_transaction2<'a, R, F>(&'a self, mut callback: F) -> LocalBoxFuture<'a, Result<R>>
    where
        R: 'a,
        F: for<'b> AsyncFnMut((&'b Database, &'b mut ClientSession)) -> Result<R> + Send + 'a,
    {
        let db = self.clone();

        async move {
            let mut session = db.client().start_session().await?;
            session
                .start_transaction()
                .and_run2(async |session| callback((&db, session)).await)
                .await
        }
        .boxed_local()
    }
}

/// A `MongoDB` database context, optionally paired with a transaction session.
///
/// Khan operations accept any `Send` implementor of this trait. Use [`Database`] or `&Database` for normal
/// operations, or `(&Database, &mut ClientSession)` when working with a raw driver session.
pub trait Mongo: Send {
    /// Returns the underlying `MongoDB` database.
    fn db(&self) -> &Database;

    /// Returns the active transactional session, if one is present.
    fn session(&mut self) -> Option<&mut ClientSession> {
        None
    }
}

impl Mongo for Database {
    fn db(&self) -> &Database {
        self
    }
}

impl Mongo for &Database {
    fn db(&self) -> &Database {
        self
    }
}

impl Mongo for (&Database, &mut ClientSession) {
    fn db(&self) -> &Database {
        self.0
    }

    fn session(&mut self) -> Option<&mut ClientSession> {
        Some(&mut *self.1)
    }
}

impl<T: Mongo + ?Sized> Mongo for &mut T {
    fn db(&self) -> &Database {
        (**self).db()
    }

    fn session(&mut self) -> Option<&mut ClientSession> {
        (**self).session()
    }
}

/// A `MongoDB` database context with an active transaction session.
///
/// Locking APIs require this stronger trait instead of plain [`Mongo`] so they cannot be called outside a
/// transaction by accident.
pub trait Transaction: Mongo {
    /// Returns the active transaction session.
    fn transaction_session(&mut self) -> &mut ClientSession;
}

impl Transaction for (&Database, &mut ClientSession) {
    fn transaction_session(&mut self) -> &mut ClientSession {
        &mut *self.1
    }
}

impl<T: Transaction + ?Sized> Transaction for &mut T {
    fn transaction_session(&mut self) -> &mut ClientSession {
        (**self).transaction_session()
    }
}

/// Applies a session to a `MongoDB` query if a session is present.
///
/// This macro is used to conditionally attach a transactional session to a query or command:
///
/// ```
/// # use khan::with_session;
/// # struct Session;
/// # struct Query {
/// #     used_session: bool,
/// # }
/// # impl Query {
/// #     fn session(self, _session: &mut Session) -> Self {
/// #         Self { used_session: true }
/// #     }
/// # }
/// let mut session = Session;
/// let result = with_session!(Query { used_session: false }, Some(&mut session));
/// assert!(result.used_session);
///
/// let result = with_session!(Query { used_session: false }, Option::<&mut Session>::None);
/// assert!(!result.used_session);
/// ```
///
/// If `session` is `Some`, it calls `.session(session)` on the query;
/// otherwise, it returns the query unchanged.
#[macro_export]
macro_rules! with_session {
    ($query: expr, $session: expr) => {
        match $session {
            Some(session) => $query.session(session),
            None => $query,
        }
    };
}

/// Trait representing a `MongoDB` query filter for a given entity type.
///
/// This trait is implemented by both typed filters (e.g. `TypedFilter`) and untyped filters
/// (e.g. `UntypedFilter`). It allows Khan methods to accept filters in a uniform way,
/// while preserving type safety where possible.
///
/// You typically don't implement this trait manually. Use:
/// - `entity::filter! { ... }` for typed filters, or
/// - `UntypedFilter::new(...)` for raw BSON-based filters.
pub trait Filter<E>: Send {
    /// Converts the filter into a MongoDB-compatible BSON document.
    fn to_document(&self) -> Result<Document>;
}

/// A simple filter that matches a document by its `id` field.
///
/// It implements [`Filter<E>`] and generates a filter of the form `{ "_id": <id> }`.
#[derive(Debug)]
pub struct FilterById<E: Entity>(E::Id, PhantomData<E>);

/// Creates a filter that matches a document by its `id`.
pub fn by_id<E: Entity>(id: E::Id) -> FilterById<E> {
    FilterById(id, PhantomData)
}

impl<E: Entity> Filter<E> for FilterById<E> {
    fn to_document(&self) -> Result<Document> {
        Ok(doc! { "_id": bson::to_bson(&self.0).map_err(mongodb::error::Error::custom)? })
    }
}

/// A raw BSON filter for an entity, bypassing Khan’s typed filter system.
///
/// Use this when you need to express advanced `MongoDB` queries that are not supported by Khan’s typed
/// filters — for example, using `$regex`, `$or`, or computed expressions.
#[derive(Debug)]
pub struct UntypedFilter<E: Send>(Document, PhantomData<E>);

impl<E: Send> UntypedFilter<E> {
    /// Creates a new untyped filter from a raw BSON document.
    pub fn new(document: Document) -> Self {
        Self(document, PhantomData)
    }
}

impl<E: Send> Filter<E> for UntypedFilter<E> {
    fn to_document(&self) -> Result<Document> {
        Ok(self.0.clone())
    }
}

/// Represents a typed `MongoDB` filter operator for a specific field.
///
/// This enum is used internally by Khan’s `TypedFilter` structs, and is also supported by the `filter!`
/// macro generated for each entity.
///
/// Variants correspond to common `MongoDB` query operators, such as `$eq`, `$ne`, `$gt`, etc.
///
/// Example:
/// ```
/// # use khan::{Field, FilterOperator};
/// # use khan::mongodb::bson::doc;
/// let field = Field::Set(FilterOperator::Gt(&10));
///
/// if let Field::Set(operator) = field {
///     assert_eq!(operator.to_document()?, doc! { "$gt": 10 });
/// }
/// # Ok::<(), mongodb::error::Error>(())
/// ```
///
/// This constructs a typed filter that translates to `{ "field": { "$gt": 10 } }`.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum FilterOperator<'a, T: Serialize + ?Sized> {
    Eq(&'a T),
    Ne(&'a T),
    Gt(&'a T),
    Gte(&'a T),
    Lt(&'a T),
    Lte(&'a T),
    In(&'a [&'a T]),
    Nin(&'a [&'a T]),
}

impl<T: Serialize + ?Sized> FilterOperator<'_, T> {
    /// Converts the filter operator into its corresponding `MongoDB` BSON document.
    pub fn to_document(&self) -> Result<Document> {
        fn to_bson<T: Serialize>(val: &T) -> Result<Bson> {
            bson::to_bson(val).map_err(mongodb::error::Error::custom)
        }

        let (operator, bson) = match self {
            Self::Eq(val) => ("$eq", to_bson(val)?),
            Self::Ne(val) => ("$ne", to_bson(val)?),
            Self::Gt(val) => ("$gt", to_bson(val)?),
            Self::Gte(val) => ("$gte", to_bson(val)?),
            Self::Lt(val) => ("$lt", to_bson(val)?),
            Self::Lte(val) => ("$lte", to_bson(val)?),
            Self::In(vals) => ("$in", to_bson(vals)?),
            Self::Nin(vals) => ("$nin", to_bson(vals)?),
        };

        Ok(doc! { operator: bson })
    }
}

/// Trait representing an update expression for a given entity type.
///
/// This trait is implemented by both typed updates (e.g. `TypedUpdate`) and untyped updates (e.g.
/// `UntypedUpdate` or `UntypedUpdateApply`).
///
/// You typically don’t implement this manually. Use:
/// - `entity::update! { ... }` for typed updates, or
/// - `UntypedUpdate::new(...)` for raw BSON updates.
pub trait Update<E>: Send {
    /// Converts the update into a BSON document.
    fn to_document(&self) -> Result<Document>;
}

/// A raw BSON update for an entity, bypassing Khan’s typed update system.
///
/// Use this when you need to express complex update operations that are not supported
/// by typed updates — such as `$inc`, `$push`, `$pop`, or updates on nested fields.
#[derive(Debug)]
pub struct UntypedUpdate<E>(Document, PhantomData<E>);

impl<E> UntypedUpdate<E> {
    /// Creates an instance of `UntypedUpdate` from a raw BSON document.
    pub fn new(document: Document) -> Self {
        Self(document, PhantomData)
    }
}

impl<E: Send> Update<E> for UntypedUpdate<E> {
    fn to_document(&self) -> Result<Document> {
        Ok(self.0.clone())
    }
}

/// Trait for applying an update to an in-memory projection.
///
/// This is used by methods like [`patch`](SelectableWithId::patch) to ensure that the changes applied to the
/// database are also reflected in the local struct.
///
/// It is typically implemented automatically for typed updates, or provided via a closure when using
/// [`UntypedUpdateApply`].
pub trait UpdateApply<S> {
    /// Applies the update to the given in-memory projection instance.
    fn apply(self, selectable: &mut S) -> Result<()>;
}

/// A raw BSON update paired with an in-memory update function.
///
/// This is used in situations where you need to perform a custom `MongoDB` update
/// (e.g. `$pop`, `$push`, `$inc`) and also reflect the same change on the local struct.
///
/// Typically used with the [`patch`](SelectableWithId::patch) and
/// [`patch_and_fence`](SelectableWithId::patch_and_fence) methods when typed updates are not sufficient.
#[derive(Debug)]
pub struct UntypedUpdateApply<E: Entity, S: Selectable<E>, F: Fn(&mut S) + Send>(
    Document,
    F,
    PhantomData<(E, S)>,
);

impl<E: Entity, S: Selectable<E>, F: Fn(&mut S) + Send> UntypedUpdateApply<E, S, F> {
    /// Creates a new untyped update with an accompanying in-memory apply function.
    ///
    /// - `document`: a raw BSON update (e.g. using `$push`, `$pop`, etc.)
    /// - `apply`: a closure that applies the same change to the in-memory struct
    pub fn new(document: Document, apply: F) -> Self {
        Self(document, apply, PhantomData)
    }
}

impl<E: Entity, S: Selectable<E>, F: Fn(&mut S) + Send> Update<E> for UntypedUpdateApply<E, S, F> {
    fn to_document(&self) -> Result<Document> {
        Ok(self.0.clone())
    }
}

impl<E: Entity, S: Selectable<E>, F: Fn(&mut S) + Send> UpdateApply<S>
    for UntypedUpdateApply<E, S, F>
{
    fn apply(self, selectable: &mut S) -> Result<()> {
        self.1(selectable);
        Ok(())
    }
}

/// Sort direction for `MongoDB` queries and index definitions.
#[derive(Debug)]
pub enum Order {
    /// MongoDB’s ascending sort (`1`)
    Asc,
    /// MongoDB’s descending sort (`-1`)
    Desc,
}

/// A wrapper used in Khan’s typed filters and updates to mark field usage.
///
/// This is used in auto-generated `TypedFilter` and `TypedUpdate` structs,
/// as well as in the `filter!` and `update!` macros.
///
/// - `Set(value)` includes the field in the filter or update.
/// - `Omit` means the field will be excluded from the generated document.
#[derive(Debug, Default)]
#[allow(missing_docs)]
pub enum Field<T> {
    Set(T),
    #[default]
    Omit,
}

impl<T> Field<T> {
    /// Converts an `Option<T>` into a `Field<T>`.
    ///
    /// - `Some(value)` becomes `Field::Set(value)`
    /// - `None` becomes `Field::Omit`
    pub fn from_opt(opt: Option<T>) -> Self {
        match opt {
            Some(val) => Self::Set(val),
            None => Self::Omit,
        }
    }
}

/// A type-level marker indicating that a document has been fenced in the current transaction.
///
/// A fence is not a mutex or SQL-style row lock. It means the transaction has written the referenced document
/// or inserted it, so a conflicting concurrent write/delete detected by `MongoDB` will prevent this
/// transaction from committing successfully.
///
/// It can be passed to any method that requires a fenced input.
///
/// `Fence<T>` implements `Deref` so it behaves like the inner value in most cases.
#[derive(Debug)]
pub struct Fence<T>(T);

impl<T> Fence<T> {
    /// Creates a new `Fence<T>`. This should only be used if you're certain the value has already been
    /// written or inserted within the current transaction.
    pub fn new_unchecked(fenced: T) -> Self {
        Self(fenced)
    }

    /// Consumes the `Fence` and returns the underlying value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<E: Entity> Fence<E> {
    /// Returns a `Fence` containing just the entity’s ID, preserving the fencing marker at the ID level.
    pub fn fenced_id(&self) -> Fence<E::Id> {
        Fence(self.0.id())
    }
}

impl<T> std::ops::Deref for Fence<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Fence<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A wrapper around `mongodb::results::UpdateResult` that represents the result of an update operation on a
/// `MongoDB` collection.
#[repr(transparent)]
#[derive(Debug)]
pub struct UpdateResult(pub mongodb::results::UpdateResult);

impl UpdateResult {
    /// Returns the wrapped `MongoDB` driver result.
    pub fn raw(&self) -> &mongodb::results::UpdateResult {
        &self.0
    }

    /// Consumes this wrapper and returns the wrapped `MongoDB` driver result.
    pub fn into_inner(self) -> mongodb::results::UpdateResult {
        self.0
    }

    /// Returns the number of documents that matched the update filter.
    pub fn matched_count(&self) -> u64 {
        self.0.matched_count
    }

    /// Returns `true` if at least one document matched the update filter.
    pub fn matched(&self) -> bool {
        self.matched_count() != 0
    }

    /// Returns the number of documents that were actually modified.
    pub fn modified_count(&self) -> u64 {
        self.0.modified_count
    }

    /// Returns `true` if at least one document was modified.
    pub fn modified(&self) -> bool {
        self.modified_count() != 0
    }
}

impl std::ops::Deref for UpdateResult {
    type Target = mongodb::results::UpdateResult;

    fn deref(&self) -> &Self::Target {
        self.raw()
    }
}

/// A wrapper around `mongodb::results::DeleteResult`, representing
/// the outcome of a delete operation.
#[repr(transparent)]
#[derive(Debug)]
pub struct DeleteResult(pub mongodb::results::DeleteResult);

impl DeleteResult {
    /// Returns the wrapped `MongoDB` driver result.
    pub fn raw(&self) -> &mongodb::results::DeleteResult {
        &self.0
    }

    /// Consumes this wrapper and returns the wrapped `MongoDB` driver result.
    pub fn into_inner(self) -> mongodb::results::DeleteResult {
        self.0
    }

    /// Returns the number of documents that were deleted.
    pub fn deleted_count(&self) -> u64 {
        self.0.deleted_count
    }

    /// Returns `true` if at least one document was deleted.
    pub fn deleted(&self) -> bool {
        self.deleted_count() != 0
    }
}

impl std::ops::Deref for DeleteResult {
    type Target = mongodb::results::DeleteResult;

    fn deref(&self) -> &Self::Target {
        self.raw()
    }
}

fn merge_fence_into_update<E>(update: &impl Update<E>) -> Result<UntypedUpdate<E>> {
    let mut document = update.to_document()?;

    let inc_operator = document
        .entry("$inc".into())
        .or_insert_with(|| doc! {}.into())
        .as_document_mut()
        .ok_or_else(|| mongodb::error::Error::custom("`$inc` operator value must be an object"))?;

    inc_operator.insert("__fence", 1);

    Ok(UntypedUpdate::new(document))
}
