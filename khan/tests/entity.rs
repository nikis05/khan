use khan::{Entity, Fields, Selectable};
use khan_macros::async_test;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utils::get_mongo;

mod utils;

mod fields {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[test]
    fn to_string_impl_is_correct() {
        assert_eq!(user::Fields::Id.to_string(), "_id");
        assert_eq!(user::Fields::Name.to_string(), "full_name");
        assert_eq!(user::Fields::Password.to_string(), "password");
    }
}

mod fields_derive {
    use super::*;

    #[test]
    fn to_string_impl_is_correct() {
        #[allow(dead_code)]
        #[derive(Serialize, Deserialize, Fields)]
        struct Profile {
            avatar_url: String,
            #[serde(rename = "full_name")]
            name: String,
        }

        assert_eq!(profile::Fields::AvatarUrl.to_string(), "avatar_url");
        assert_eq!(profile::Fields::Name.to_string(), "full_name");
    }
}

mod collection_name {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[test]
    fn defaults_to_lowercase_struct_name() {
        assert_eq!(User::COLLECTION_NAME, "user");
    }

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation, collection = "users")]
    struct User2 {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[test]
    fn can_be_overriden() {
        assert_eq!(User2::COLLECTION_NAME, "users");
    }
}

mod count {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[khan_macros::async_test]
    async fn zero() {
        let count = User::count(
            get_mongo(),
            user::filter! {
                name: &fakeit::name::full()
            },
        )
        .await
        .unwrap();

        assert_eq!(count, 0);
    }

    #[async_test]
    async fn multiple() {
        let name_1 = fakeit::name::full();
        let name_2 = fakeit::name::full();
        let password = fakeit::password::generate(true, true, true, 16);

        User::insert_many(
            get_mongo(),
            &[
                User {
                    id: ObjectId::new(),
                    name: name_1,
                    password: password.clone(),
                },
                User {
                    id: ObjectId::new(),
                    name: name_2.clone(),
                    password: password.clone(),
                },
                User {
                    id: ObjectId::new(),
                    name: name_2.clone(),
                    password,
                },
            ],
        )
        .await
        .unwrap();

        let count = User::count(
            get_mongo(),
            user::filter! {
                name: &name_2
            },
        )
        .await
        .unwrap();

        assert_eq!(count, 2);
    }
}

mod exists {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn doesnt_exist() {
        let name = fakeit::name::full();
        assert!(
            !User::exists(
                get_mongo(),
                user::filter! {
                    name: &name
                },
            )
            .await
            .unwrap()
        );
    }

    #[async_test]
    async fn exists() {
        let name = fakeit::name::full();

        User {
            id: ObjectId::new(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        }
        .insert(get_mongo())
        .await
        .unwrap();

        assert!(
            User::exists(
                get_mongo(),
                user::filter! {
                    name: &name
                },
            )
            .await
            .unwrap()
        );
    }
}

mod insert {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, Debug, PartialEq, Eq)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn normal() {
        let user = User {
            id: ObjectId::new(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let found_user = User::find_one(get_mongo(), user::filter! { id: &user.id })
            .await
            .unwrap();

        assert_eq!(found_user, Some(user));
    }
}

mod insert_many {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, Debug, PartialEq, Eq)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn multiple() {
        let gen_user = || User {
            id: ObjectId::new(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let user_ids = users.iter().map(|user| &user.id).collect::<Vec<_>>();

        let found_users = User::find(get_mongo(), user::filter! { id: In(&user_ids) })
            .await
            .unwrap();

        assert_eq!(users.len(), found_users.len());

        for found_user in found_users {
            let matching_user = users.iter().find(|user| user.id == found_user.id).unwrap();
            assert_eq!(found_user, *matching_user);
        }
    }

    #[async_test]
    async fn empty() {
        User::insert_many(get_mongo(), &[]).await.unwrap();
    }
}

mod update {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn matched_modified() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let new_password = &users[0].password;

        let result = User::update(
            get_mongo(),
            user::filter! { name: &name},
            user::update! { password: new_password.clone() },
        )
        .await
        .unwrap();

        assert!(result.matched());
        assert!(result.modified());
        assert_eq!(result.matched_count(), 3);
        assert_eq!(result.modified_count(), 2);

        let found_users = User::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert!(
            found_users
                .iter()
                .all(|user| user.password == *new_password)
        );
    }

    #[async_test]
    async fn unmatched() {
        let result = User::update(
            get_mongo(),
            user::filter! { id: &ObjectId::new() },
            user::update! { name: "New name".into() },
        )
        .await
        .unwrap();

        assert!(!result.matched());
        assert!(!result.modified());
        assert_eq!(result.matched_count(), 0);
        assert_eq!(result.modified_count(), 0);
    }

    #[async_test]
    async fn unmodified() {
        let name = fakeit::name::full();
        let password = fakeit::password::generate(true, true, true, 16);

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: password.clone(),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let result = User::update(
            get_mongo(),
            user::filter! { name: &name},
            user::update! { password: password },
        )
        .await
        .unwrap();

        assert!(result.matched());
        assert!(!result.modified());
        assert_eq!(result.matched_count(), 3);
        assert_eq!(result.modified_count(), 0);
    }
}

mod update_one {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn matched_modified() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let new_password = fakeit::password::generate(true, true, true, 16);

        let result = User::update_one(
            get_mongo(),
            user::filter! { name: &name },
            user::update! { password: new_password.clone() },
        )
        .await
        .unwrap();

        assert!(result.matched());
        assert!(result.modified());
        assert_eq!(result.matched_count(), 1);
        assert_eq!(result.modified_count(), 1);

        let found_users = User::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert_eq!(
            found_users
                .iter()
                .filter(|found_user| found_user.password == new_password)
                .count(),
            1
        );

        assert_eq!(
            found_users
                .iter()
                .filter(|found_user| {
                    found_user.password
                        == users
                            .iter()
                            .find(|user| user.id == found_user.id)
                            .unwrap()
                            .password
                })
                .count(),
            2
        );
    }

    #[async_test]
    async fn unmatched() {
        let result = User::update_one(
            get_mongo(),
            user::filter! { name: &fakeit::name::full() },
            user::update! { password: fakeit::password::generate(true, true, true, 16) },
        )
        .await
        .unwrap();

        assert!(!result.matched());
        assert!(!result.modified());
        assert_eq!(result.matched_count(), 0);
        assert_eq!(result.modified_count(), 0);
    }

    #[async_test]
    async fn unmodified() {
        let name = fakeit::name::full();
        let password = fakeit::password::generate(true, true, true, 16);

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: password.clone(),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let result = User::update_one(
            get_mongo(),
            user::filter! { name: &name },
            user::update! { password: password },
        )
        .await
        .unwrap();

        assert!(result.matched());
        assert!(!result.modified());
        assert_eq!(result.matched_count(), 1);
        assert_eq!(result.modified_count(), 0);
    }
}

mod delete {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn deleted() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let result = User::delete(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert!(result.deleted());
        assert_eq!(result.deleted_count(), 3);

        let found = User::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert!(found.is_empty());
    }

    #[async_test]
    async fn not_deleted() {
        let result = User::delete(get_mongo(), user::filter! { name: &fakeit::name::full() })
            .await
            .unwrap();

        assert!(!result.deleted());
        assert_eq!(result.deleted_count(), 0);
    }
}

mod delete_one {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation)]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn deleted() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let result = User::delete_one(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert!(result.deleted());
        assert_eq!(result.deleted_count(), 1);

        let found = User::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert_eq!(found.len(), 2);
    }

    #[async_test]
    async fn not_deleted() {
        let result = User::delete_one(get_mongo(), user::filter! { name: &fakeit::name::full() })
            .await
            .unwrap();

        assert!(!result.deleted());
        assert_eq!(result.deleted_count(), 0);
    }
}
