//! Khan is a `MongoDB` ORM for Rust.
//!
//! ## Example
//!
//! ```
//! // Define an entity
//! #[derive(Serialize, Deserialize, Entity)]
//! #[entity(projections(Profile(email, password)))]
//! struct User {
//!   email: String,
//!   username: String,
//!   password: String,
//!   created_at: chrono::DateTime<chrono::Utc>,
//! }
//!
//! // Select an entity by id
//! let person: User = User::find_one(mongo, by_id(user_id)).await?;
//!
//! // Select an entity by custom fields
//! let recent_user: User = User::find_one(mongo, user::filter! {
//!   created_at: Gt(Utc::now() - Duration::hours(1)),
//! }).await?;
//!  
//! // Select only necessary fields (email, password) of entity
//! let profile: Profile = Profile::find_one(mongo, by_id(user_id)).await?;
//!
//! // Insert an entity into the database
//! let user = User {
//!   email: "mail@example.com".into(),
//!   username: "nikis05".into(),
//!   password: "somepassword".into(),
//!   created_at: chrono::Utc::now(),
//! };
//!
//! user.insert(mongo).await?;
//!
//! // Update an entity in the database
//!
//! User::update_one(mongo, by_id(user_id), user::patch! {
//!   email: "new.email@example.com"
//! }).await?;
//!
//! // Update an entity in the database (struct is automatically updated)
//!
//! user.patch(mongo, user::patch! {
//!   email: "new.email@example.com".into(),
//!   password: "someotherpassword".into()
//! }).await?;
//!
//! // Delete entities matching the filter
//! User::delete_one(mongo, by_id(user_id)).await?;
//!
//! // Remove a document from the database that corresponds to an instance
//! user.remove(mongo).await?;
//! ```
//!
//! See [`guides`] module to learn more!

#![warn(clippy::pedantic)]
#![allow(
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::missing_errors_doc
)]

use futures_util::{FutureExt, TryStreamExt, future::BoxFuture};
use mongodb::{
    ClientSession, Collection, Database,
    bson::{self, Bson, Document, bson, doc, oid::ObjectId},
    error::Result,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fmt::Display, marker::PhantomData, sync::LazyLock};

pub use khan_macros::{Entity, construct_filter, construct_update};

pub mod guides;

pub trait Entity: ProjectionWithId<Self> + Serialize {
    type Id: Copy + Serialize + Send + 'static;

    type Fields: Display + Send + 'static;

    const COLLECTION_NAME: &'static str;

    fn collection(db: &Database) -> Collection<Self> {
        db.collection(Self::COLLECTION_NAME)
    }

    fn count<'a>(mongo: Mongo<'a>, filter: impl Filter<Self> + 'a) -> BoxFuture<'a, Result<u64>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            let count =
                with_session!(collection.count_documents(filter.to_document()), session).await?;

            Ok(count)
        }
        .boxed()
    }

    fn exists<'a>(mongo: Mongo<'a>, filter: impl Filter<Self> + 'a) -> BoxFuture<'a, Result<bool>> {
        async move {
            let count = Self::count(mongo, filter).await?;

            Ok(count > 0)
        }
        .boxed()
    }

    fn insert<'a>(&'a self, mongo: Mongo<'a>) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(collection.insert_one(self), session).await?;

            Ok(())
        }
        .boxed()
    }

    fn insert_locked(self, trx: Transaction<'_>) -> BoxFuture<'_, Result<Lock<Self>>> {
        async move {
            Self::insert(&self, trx.into()).await?;

            Ok(Lock(self))
        }
        .boxed()
    }

    fn insert_many<'a>(mongo: Mongo<'a>, entities: &'a [Self]) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(collection.insert_many(entities), session).await?;

            Ok(())
        }
        .boxed()
    }

    fn insert_many_locked(
        trx: Transaction<'_>,
        entities: Vec<Self>,
    ) -> BoxFuture<'_, Result<Vec<Lock<Self>>>> {
        async move {
            Self::insert_many(trx.into(), &entities).await?;

            Ok(entities.into_iter().map(Lock).collect())
        }
        .boxed()
    }

    fn update<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<Self> + 'a,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(
                collection.update_many(filter.to_document(), doc! { "$set": update.to_document() }),
                session
            )
            .await?;

            Ok(())
        }
        .boxed()
    }

    fn update_one<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<Self> + 'a,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(
                collection.update_one(filter.to_document(), doc! { "$set": update.to_document() }),
                session
            )
            .await?;

            Ok(())
        }
        .boxed()
    }

    fn update_by_id_locked<'a>(
        trx: Transaction<'a>,
        id: Self::Id,
        update: impl Update<Self> + 'a,
    ) -> BoxFuture<'a, Result<Lock<Self::Id>>> {
        async move {
            Self::update_one(trx.into(), by_id(id), update).await?;

            Ok(Lock(id))
        }
        .boxed()
    }

    fn lock_by_id(trx: Transaction<'_>, id: Self::Id) -> BoxFuture<'_, Result<Lock<Self::Id>>> {
        Self::update_by_id_locked(
            trx,
            id,
            UntypedUpdate::new(doc! { "$set": { "_lock": { "seed": ObjectId::new() } } }),
        )
    }

    fn delete<'a>(mongo: Mongo<'a>, filter: impl Filter<Self> + 'a) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(collection.delete_many(filter.to_document()), session).await?;

            Ok(())
        }
        .boxed()
    }

    fn delete_one<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<Self> + 'a,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = Self::collection(db);

            with_session!(collection.delete_one(filter.to_document()), session).await?;

            Ok(())
        }
        .boxed()
    }
}

pub trait Projection<E: Entity>: DeserializeOwned + Send + Sync + 'static {
    const FIELDS: Option<&'static [&'static str]>;

    fn projection_document() -> Option<Document> {
        static DOCUMENTS: LazyLock<dashmap::DashMap<&'static [&'static str], Document>> =
            LazyLock::new(dashmap::DashMap::new);

        Self::FIELDS.map(|fields| {
            if let Some(document) = DOCUMENTS.get(fields) {
                document.clone()
            } else {
                let mut has_id = false;
                let mut document = doc! {};

                for field in fields {
                    if *field == "id" {
                        has_id = true;
                    } else {
                        document.insert(*field, 1);
                    }
                }

                if !has_id {
                    document.insert("_id", -1);
                }

                DOCUMENTS.insert(fields, document.clone());
                document
            }
        })
    }

    fn find_with_opts<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<E> + 'a,
        skip: Option<u64>,
        limit: Option<i64>,
        sort: Option<BTreeMap<E::Fields, Order>>,
    ) -> BoxFuture<'a, Result<Vec<Self>>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = db.collection(E::COLLECTION_NAME);

            let mut query = collection.find(filter.to_document());

            if let Some(projection) = Self::projection_document() {
                query = query.projection(projection);
            }

            if let Some(skip) = skip {
                query = query.skip(skip);
            }

            if let Some(limit) = limit {
                query = query.limit(limit);
            }

            if let Some(sort) = sort {
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

            let entities = match session {
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

    fn find<'a>(mongo: Mongo<'a>, filter: impl Filter<E> + 'a) -> BoxFuture<'a, Result<Vec<Self>>> {
        Self::find_with_opts(mongo, filter, None, None, None)
    }

    fn find_one<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Self>>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = db.collection(E::COLLECTION_NAME);

            let mut query = collection.find_one(filter.to_document());
            if let Some(projection) = Self::projection_document() {
                query = query.projection(projection);
            }

            let entity = with_session!(query, session).await?;

            Ok(entity)
        }
        .boxed()
    }

    fn find_one_and_lock<'a>(
        trx: Transaction<'a>,
        filter: impl Filter<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Lock<Self>>>> {
        Self::find_one_and_update_locked(
            trx,
            filter,
            UntypedUpdate::new(doc! { "$set": { "_lock": { "seed": ObjectId::new() } } }),
        )
    }

    fn find_one_and_update<'a>(
        mongo: Mongo<'a>,
        filter: impl Filter<E> + 'a,
        update: impl Update<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Self>>> {
        async move {
            let Mongo { db, session } = mongo;
            let collection = db.collection(E::COLLECTION_NAME);

            let mut query =
                collection.find_one_and_update(filter.to_document(), update.to_document());
            if let Some(projection) = Self::projection_document() {
                query = query.projection(projection);
            }

            let entity = with_session!(query, session).await?;

            Ok(entity)
        }
        .boxed()
    }

    fn find_one_and_update_locked<'a>(
        trx: Transaction<'a>,
        filter: impl Filter<E> + 'a,
        update: impl Update<E> + 'a,
    ) -> BoxFuture<'a, Result<Option<Lock<Self>>>> {
        async move {
            let entity = Self::find_one_and_update(trx.into(), filter, update).await?;

            Ok(entity.map(Lock))
        }
        .boxed()
    }
}

pub trait ProjectionWithId<E: Entity>: Projection<E> {
    fn id(&self) -> E::Id;

    fn patch<'a>(
        &'a mut self,
        mongo: Mongo<'a>,
        update: impl Update<E> + UpdateApply<Self> + 'a,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            E::update_one(
                mongo,
                by_id(self.id()),
                UntypedUpdateApply::new(update.to_document(), |_: &mut Self| {}),
            )
            .await?;

            update.apply(self)?;

            Ok(())
        }
        .boxed()
    }

    fn patch_locked<'a>(
        mut self,
        trx: Transaction<'a>,
        update: impl Update<E> + UpdateApply<Self> + 'a,
    ) -> BoxFuture<'a, Result<Lock<Self>>> {
        async move {
            self.patch(trx.into(), update).await?;

            Ok(Lock(self))
        }
        .boxed()
    }

    fn remove<'a>(&'a self, mongo: Mongo<'a>) -> BoxFuture<'a, Result<()>> {
        async move {
            E::delete_one(mongo, by_id(self.id())).await?;

            Ok(())
        }
        .boxed()
    }
}

#[derive(Debug)]
pub struct Mongo<'a> {
    pub db: &'a Database,
    pub session: Option<&'a mut ClientSession>,
}

impl<'a> Mongo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db, session: None }
    }

    pub fn new_with_session(db: &'a Database, session: &'a mut ClientSession) -> Self {
        Self {
            db,
            session: Some(session),
        }
    }

    pub fn rb(&mut self) -> Mongo<'_> {
        Mongo {
            db: self.db,
            session: self.session.as_deref_mut(),
        }
    }
}

impl<'a> From<&'a Database> for Mongo<'a> {
    fn from(value: &'a Database) -> Self {
        Self::new(value)
    }
}

impl<'a> From<(&'a Database, &'a mut ClientSession)> for Mongo<'a> {
    fn from(value: (&'a Database, &'a mut ClientSession)) -> Self {
        Self::new_with_session(value.0, value.1)
    }
}

#[derive(Debug)]
pub struct Transaction<'a> {
    pub db: &'a Database,
    pub session: &'a mut ClientSession,
}

impl<'a> Transaction<'a> {
    pub fn new(db: &'a Database, session: &'a mut ClientSession) -> Self {
        Self { db, session }
    }

    pub fn rb(&mut self) -> Transaction<'_> {
        Transaction {
            db: self.db,
            session: &mut *self.session,
        }
    }
}

impl<'a> From<(&'a Database, &'a mut ClientSession)> for Transaction<'a> {
    fn from(value: (&'a Database, &'a mut ClientSession)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl<'a> From<Transaction<'a>> for Mongo<'a> {
    fn from(value: Transaction<'a>) -> Self {
        Mongo {
            db: value.db,
            session: Some(value.session),
        }
    }
}

#[macro_export]
macro_rules! with_session {
    ($query: expr, $session: expr) => {
        match $session {
            Some(session) => $query.session(session),
            None => $query,
        }
    };
}

pub trait Filter<E>: Send {
    fn to_document(&self) -> Document;
}

#[derive(Debug)]
pub struct FilterById<E: Entity>(E::Id, PhantomData<E>);

pub fn by_id<E: Entity>(id: E::Id) -> FilterById<E> {
    FilterById(id, PhantomData)
}

impl<E: Entity> Filter<E> for FilterById<E> {
    fn to_document(&self) -> Document {
        doc! { "_id": bson::to_bson(&self.0).unwrap() }
    }
}

#[derive(Debug)]
pub struct UntypedFilter<E: Send>(Document, PhantomData<E>);

impl<E: Send> UntypedFilter<E> {
    pub fn new(document: Document) -> Self {
        Self(document, PhantomData)
    }
}

impl<E: Send> Filter<E> for UntypedFilter<E> {
    fn to_document(&self) -> Document {
        self.0.clone()
    }
}

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
    pub fn to_document(&self) -> Document {
        fn to_bson<T: Serialize>(val: &T) -> Bson {
            bson::to_bson(val).unwrap()
        }

        let (operator, bson) = match self {
            Self::Eq(val) => ("$eq", to_bson(val)),
            Self::Ne(val) => ("$ne", to_bson(val)),
            Self::Gt(val) => ("$gt", to_bson(val)),
            Self::Gte(val) => ("$gte", to_bson(val)),
            Self::Lt(val) => ("$lt", to_bson(val)),
            Self::Lte(val) => ("$lte", to_bson(val)),
            Self::In(vals) => ("$in", to_bson(vals)),
            Self::Nin(vals) => ("$nin", to_bson(vals)),
        };

        doc! { operator: bson }
    }
}

pub trait Update<E>: Send {
    fn to_document(&self) -> Document;
}

#[derive(Debug)]
pub struct UntypedUpdate<E>(Document, PhantomData<E>);

impl<E> UntypedUpdate<E> {
    fn new(document: Document) -> Self {
        Self(document, PhantomData)
    }
}

impl<E: Send> Update<E> for UntypedUpdate<E> {
    fn to_document(&self) -> Document {
        self.0.clone()
    }
}

pub trait UpdateApply<P> {
    fn apply(self, projection: &mut P) -> Result<()>;
}

#[derive(Debug)]
pub struct UntypedUpdateApply<E: Entity, P: Projection<E>, F: Fn(&mut P) + Send>(
    Document,
    F,
    PhantomData<(E, P)>,
);

impl<E: Entity, P: Projection<E>, F: Fn(&mut P) + Send> UntypedUpdateApply<E, P, F> {
    pub fn new(document: Document, apply: F) -> Self {
        Self(document, apply, PhantomData)
    }
}

impl<E: Entity, P: Projection<E>, F: Fn(&mut P) + Send> Update<E> for UntypedUpdateApply<E, P, F> {
    fn to_document(&self) -> Document {
        self.0.clone()
    }
}

impl<E: Entity, P: Projection<E>, F: Fn(&mut P) + Send> UpdateApply<P>
    for UntypedUpdateApply<E, P, F>
{
    fn apply(self, projection: &mut P) -> Result<()> {
        self.1(projection);
        Ok(())
    }
}

#[derive(Debug)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug)]
pub enum Field<T> {
    Set(T),
    Omit,
}

impl<T> Field<T> {
    pub fn from_opt(opt: Option<T>) -> Self {
        match opt {
            Some(val) => Self::Set(val),
            None => Self::Omit,
        }
    }
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Self::Omit
    }
}

#[derive(Debug)]
pub struct Lock<T>(T);

impl<T> Lock<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for Lock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Lock<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

mod example {
    use super::{Entity, Mongo, Projection, Result, by_id};
    use mongodb::bson::oid::ObjectId;
    use serde::{Deserialize, Serialize};

    // Define an entity
    #[derive(Serialize, Deserialize, Entity)]
    #[entity(projections(Profile(email, password)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        username: String,
        password: String,
        //created_at: chrono::DateTime<chrono::Utc>,
    }

    async fn test(mut mongo: Mongo<'_>, user_id: mongodb::bson::oid::ObjectId) -> Result<()> {
        // Select an entity by id
        let person: Option<User> = User::find_one(mongo.rb(), by_id(user_id)).await?;

        // Select an entity by custom fields
        let me: Option<User> = User::find_one(
            mongo.rb(),
            user::filter! {
              username: "Kit",
            },
        )
        .await?;

        // Select only necessary fields (email, password) of entity
        let profile: Option<user::Profile> =
            user::Profile::find_one(mongo.rb(), by_id(user_id)).await?;

        // Insert an entity into the database
        let user = User {
            id: ObjectId::new(),
            email: "mail@example.com".into(),
            username: "nikis05".into(),
            password: "somepassword".into(),
        };

        user.insert(mongo.rb()).await?;

        Ok(())
    }
}
