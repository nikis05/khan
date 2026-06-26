#[path = "../../src/test_support.rs"]
mod shared;

pub use shared::{RUNTIME, mongo};

#[ctor::ctor]
fn constructor() {
    shared::initialize();
}

pub fn get_mongo() -> &'static khan::mongodb::Database {
    mongo()
}
