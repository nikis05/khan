use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{self, Bson, Document, bson, doc, oid::ObjectId},
    error::Result,
};
use once_cell::sync::Lazy;
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fmt::Display, marker::PhantomData};

pub trait Entity: ProjectionWithId<Self> + Serialize {
    type Id: Copy + Serialize;

    type Fields: Display;

    const COLLECTION_NAME: &'static str;

    fn collection(db: &Database) -> Collection<Self> {
        db.collection(Self::COLLECTION_NAME)
    }

    async fn count(mongo: Mongo<'_>, filter: impl Filter<Self>) -> Result<u64> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        let count =
            with_session!(collection.count_documents(filter.to_document()), session).await?;

        Ok(count)
    }

    async fn exists(mongo: Mongo<'_>, filter: impl Filter<Self>) -> Result<bool> {
        let count = Self::count(mongo, filter).await?;

        Ok(count > 0)
    }

    async fn insert(&self, mongo: Mongo<'_>) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(collection.insert_one(self), session).await?;

        Ok(())
    }

    async fn insert_locked(self, trx: Transaction<'_>) -> Result<Lock<Self>> {
        let (db, session) = trx;
        Self::insert(&self, (db, Some(session))).await?;

        Ok(Lock(self))
    }

    async fn insert_many(mongo: Mongo<'_>, entities: &[Self]) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(collection.insert_many(entities), session).await?;

        Ok(())
    }

    async fn insert_many_locked(
        trx: Transaction<'_>,
        entities: Vec<Self>,
    ) -> Result<Vec<Lock<Self>>> {
        let (db, session) = trx;
        Self::insert_many((db, Some(session)), &entities).await?;

        Ok(entities.into_iter().map(Lock).collect())
    }

    async fn update(
        mongo: Mongo<'_>,
        filter: impl Filter<Self>,
        update: impl Update<Self>,
    ) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(
            collection.update_many(filter.to_document(), doc! { "$set": update.to_document() }),
            session
        )
        .await?;

        Ok(())
    }

    async fn update_one(
        mongo: Mongo<'_>,
        filter: impl Filter<Self>,
        update: impl Update<Self>,
    ) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(
            collection.update_one(filter.to_document(), doc! { "$set": update.to_document() }),
            session
        )
        .await?;

        Ok(())
    }

    async fn update_by_id_locked(
        trx: Transaction<'_>,
        id: Self::Id,
        update: impl Update<Self>,
    ) -> Result<Lock<Self::Id>> {
        let (db, session) = trx;

        Self::update_one((db, Some(session)), FilterById::new(id), update).await?;

        Ok(Lock(id))
    }

    async fn lock_by_id(trx: Transaction<'_>, id: Self::Id) -> Result<Lock<Self::Id>> {
        Self::update_by_id_locked(
            trx,
            id,
            UntypedUpdate::new(
                doc! { "$set": { "_lock": { "seed": ObjectId::new() } } },
                |_| {},
            ),
        )
        .await
    }

    async fn delete(mongo: Mongo<'_>, filter: impl Filter<Self>) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(collection.delete_many(filter.to_document()), session).await?;

        Ok(())
    }

    async fn delete_one(mongo: Mongo<'_>, filter: impl Filter<Self>) -> Result<()> {
        let (db, session) = mongo;
        let collection = Self::collection(db);

        with_session!(collection.delete_one(filter.to_document()), session).await?;

        Ok(())
    }
}

pub trait Projection<E: Entity>: DeserializeOwned + Send + Sync {
    const FIELDS: Option<&'static [&'static str]>;

    fn projection_document() -> Option<Document> {
        static DOCUMENTS: Lazy<dashmap::DashMap<&'static [&'static str], Document>> =
            Lazy::new(|| dashmap::DashMap::new());

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

    async fn find_with_opts(
        mongo: Mongo<'_>,
        filter: impl Filter<E>,
        skip: Option<u64>,
        limit: Option<i64>,
        sort: Option<BTreeMap<E::Fields, Order>>,
    ) -> Result<Vec<Self>> {
        let (db, session) = mongo;
        let collection = db.collection(E::COLLECTION_NAME);

        let mut query = collection.find(filter.to_document());

        if let Some(projection) = Self::projection_document() {
            query = query.projection(projection);
        }

        if let Some(skip) = skip {
            query = query.skip(skip);
        };

        if let Some(limit) = limit {
            query = query.limit(limit);
        };

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
        };

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

    async fn find(mongo: Mongo<'_>, filter: impl Filter<E>) -> Result<Vec<Self>> {
        Self::find_with_opts(mongo, filter, None, None, None).await
    }

    async fn find_one(mongo: Mongo<'_>, filter: impl Filter<E>) -> Result<Option<Self>> {
        let (db, session) = mongo;
        let collection = db.collection(E::COLLECTION_NAME);

        let mut query = collection.find_one(filter.to_document());
        if let Some(projection) = Self::projection_document() {
            query = query.projection(projection);
        }

        let entity = with_session!(query, session).await?;

        Ok(entity)
    }

    async fn find_one_and_lock(
        trx: Transaction<'_>,
        filter: impl Filter<E>,
    ) -> Result<Option<Lock<Self>>> {
        Self::find_one_and_update_locked(
            trx,
            filter,
            UntypedUpdate::new(
                doc! { "$set": { "_lock": { "seed": ObjectId::new() } } },
                |_| {},
            ),
        )
        .await
    }

    async fn find_one_and_update(
        mongo: Mongo<'_>,
        filter: impl Filter<E>,
        update: impl Update<E>,
    ) -> Result<Option<Self>> {
        let (db, session) = mongo;
        let collection = db.collection(E::COLLECTION_NAME);

        let mut query = collection.find_one_and_update(filter.to_document(), update.to_document());
        if let Some(projection) = Self::projection_document() {
            query = query.projection(projection);
        }

        let entity = with_session!(query, session).await?;

        Ok(entity)
    }

    async fn find_one_and_update_locked(
        trx: Transaction<'_>,
        filter: impl Filter<E>,
        update: impl Update<E>,
    ) -> Result<Option<Lock<Self>>> {
        let (db, session) = trx;

        let entity = Self::find_one_and_update((db, Some(session)), filter, update).await?;

        Ok(entity.map(Lock))
    }
}

pub trait ProjectionWithId<E: Entity>: Projection<E> {
    fn id(&self) -> E::Id;

    async fn patch(
        &mut self,
        mongo: Mongo<'_>,
        update: impl Update<E> + UpdateApply<Self>,
    ) -> Result<()> {
        E::update_one(
            mongo,
            FilterById::new(self.id()),
            UntypedUpdate::new(update.to_document(), |_| {}),
        )
        .await?;

        update.apply(self);

        Ok(())
    }

    async fn patch_locked(
        mut self,
        trx: Transaction<'_>,
        update: impl Update<E> + UpdateApply<Self>,
    ) -> Result<Lock<Self>> {
        let (db, session) = trx;

        self.patch((db, Some(session)), update).await?;

        Ok(Lock(self))
    }

    async fn remove(&self, mongo: Mongo<'_>) -> Result<()> {
        E::delete_one(mongo, FilterById::new(self.id())).await?;

        Ok(())
    }
}

pub type Mongo<'a> = (&'a Database, Option<&'a mut mongodb::ClientSession>);

pub type Transaction<'a> = (&'a Database, &'a mut mongodb::ClientSession);

#[macro_export]
macro_rules! with_session {
    ($query: expr, $session: expr) => {
        match $session {
            Some(session) => $query.session(session),
            None => $query,
        }
    };
}

pub trait Filter<E> {
    fn to_document(&self) -> Document;
}

pub struct FilterById<E: Entity>(E::Id, PhantomData<*const E>);

impl<E: Entity> FilterById<E> {
    pub fn new(id: E::Id) -> Self {
        Self(id, PhantomData)
    }
}

impl<E: Entity> Filter<E> for FilterById<E> {
    fn to_document(&self) -> Document {
        doc! { "_id": bson::to_bson(&self.0).unwrap() }
    }
}

pub struct UntypedFilter<E>(Document, PhantomData<E>);

impl<E> UntypedFilter<E> {
    pub fn new(document: Document) -> Self {
        Self(document, PhantomData)
    }
}

impl<E> Filter<E> for UntypedFilter<E> {
    fn to_document(&self) -> Document {
        self.0.clone()
    }
}

pub enum FilterOperator<'a, T> {
    Eq(&'a T),
    Ne(&'a T),
    Gt(&'a T),
    Gte(&'a T),
    Lt(&'a T),
    Lte(&'a T),
    In(&'a [T]),
    Nin(&'a [T]),
}

impl<T: Serialize> FilterOperator<'_, T> {
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

pub trait Update<E> {
    fn to_document(&self) -> Document;
}

pub trait UpdateApply<P> {
    fn apply(self, projection: &mut P);
}

pub struct UntypedUpdate<E: Entity, P: Projection<E>, F: Fn(&mut P)>(
    Document,
    F,
    PhantomData<*const (E, P)>,
);

impl<E: Entity, P: Projection<E>, F: Fn(&mut P)> UntypedUpdate<E, P, F> {
    pub fn new(document: Document, apply: F) -> Self {
        Self(document, apply, PhantomData)
    }
}

impl<E: Entity, P: Projection<E>, F: Fn(&mut P)> Update<P> for UntypedUpdate<E, P, F> {
    fn to_document(&self) -> Document {
        self.0.clone()
    }
}

impl<E: Entity, P: Projection<E>, F: Fn(&mut P)> UpdateApply<P> for UntypedUpdate<E, P, F> {
    fn apply(self, projection: &mut P) {
        self.1(projection);
    }
}

pub enum Order {
    Asc,
    Desc,
}

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
