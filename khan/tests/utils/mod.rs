use ctor::ctor;
use khan::Mongo;
use mongodb::{Database, options::ServerAddress};
use std::sync::{LazyLock, OnceLock};
use testcontainers::{ImageExt, runners::AsyncRunner};

pub static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
});

static CONTAINER: OnceLock<testcontainers::ContainerAsync<testcontainers_modules::mongo::Mongo>> =
    OnceLock::new();

static DATABASE: OnceLock<Database> = OnceLock::new();

#[ctor]
fn constructor() {
    RUNTIME.block_on(async {
        let container = testcontainers_modules::mongo::Mongo::repl_set()
            .with_reuse(testcontainers::ReuseDirective::Always)
            .start()
            .await
            .unwrap();

        let mongo_host = container.get_host().await.unwrap().to_string();
        let mongo_port = container.get_host_port_ipv4(27017).await.unwrap();

        CONTAINER.set(container).unwrap();
        let client = mongodb::Client::with_options(
            mongodb::options::ClientOptions::builder()
                .hosts(vec![ServerAddress::Tcp {
                    host: mongo_host,
                    port: Some(mongo_port),
                }])
                .direct_connection(true)
                .build(),
        )
        .unwrap();

        let database = client.database("test");
        DATABASE.set(database).unwrap();
    })
}

pub fn get_mongo() -> Mongo<'static> {
    DATABASE.get().unwrap().into()
}
