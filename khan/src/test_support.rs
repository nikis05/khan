#![allow(dead_code)]

use std::sync::{LazyLock, OnceLock};

use mongodb::{
    Database,
    bson::{doc, oid::ObjectId},
};

#[path = "test_entities.rs"]
mod test_entities;

#[allow(unused_imports)]
pub use test_entities::*;

/// Runtime shared by tests and doctests.
pub static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
});

static DATABASE: OnceLock<Database> = OnceLock::new();

fn database() -> &'static Database {
    DATABASE.get_or_init(|| {
        RUNTIME.block_on(async {
            let client = mongodb::Client::with_uri_str(mongodb_uri()).await.unwrap();
            client
                .database("admin")
                .run_command(doc! { "ping": 1 })
                .await
                .expect("MongoDB test database is not reachable; run `just mongo-up` or set KHAN_TEST_MONGODB_URI");

            let database_name = format!(
                "khan_test_{}_{}",
                std::process::id(),
                ObjectId::new().to_hex()
            );

            client.database(&database_name)
        })
    })
}

/// Returns a `MongoDB` database backed by the shared test fixture.
pub fn mongo() -> &'static Database {
    database()
}

/// Starts the shared fixture before entering an async test runtime.
pub fn initialize() {
    let _ = database();
}

fn mongodb_uri() -> String {
    std::env::var("KHAN_TEST_MONGODB_URI").unwrap_or_else(|_| {
        "mongodb://127.0.0.1:27017/?directConnection=true&serverSelectionTimeoutMS=1000".into()
    })
}
