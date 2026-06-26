use khan::{Entity, Selectable, SelectableWithId, by_id};
use khan_macros::async_test;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utils::get_mongo;

mod utils;

mod fields {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        skip_schema_validation,
        projections(BasicInfo(id, name), LogInInfo(id, email, password))
    )]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[test]
    fn entity() {
        assert_eq!(User::FIELDS, None);
    }

    #[test]
    fn projections() {
        assert_eq!(user::BasicInfo::FIELDS, Some(["_id", "full_name"].as_ref()));
        assert_eq!(
            user::LogInInfo::FIELDS,
            Some(["_id", "email", "password"].as_ref())
        );
    }
}

mod projection {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(
        skip_schema_validation,
        projections(
            BasicInfo(id, name),
            LogInInfo(id, email, password),
            Password(password)
        )
    )]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[test]
    fn entity() {
        assert_eq!(User::projection(), None);
    }

    #[test]
    fn projections() {
        assert_eq!(
            user::BasicInfo::projection(),
            Some(mongodb::bson::doc! { "full_name": 1 })
        );
        assert_eq!(
            user::LogInInfo::projection(),
            Some(mongodb::bson::doc! { "email": 1, "password": 1 })
        );
        assert_eq!(
            user::Password::projection(),
            Some(mongodb::bson::doc! { "_id": 0, "password": 1 })
        );
    }
}

mod find {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, PartialEq, Eq, Debug)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn entity() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_users = User::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert_eq!(found_users.len(), users.len());
        assert!(
            found_users.iter().all(|found_user| found_user
                == users.iter().find(|user| user.id == found_user.id).unwrap())
        )
    }

    #[async_test]
    async fn projection() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_profiles = user::Profile::find(get_mongo(), user::filter! { name: &name })
            .await
            .unwrap();

        assert_eq!(found_profiles.len(), users.len());
        assert!(found_profiles.iter().all(|found_profile| {
            let matching_user = users
                .iter()
                .find(|user| user.id == found_profile.id)
                .unwrap();
            found_profile.email == matching_user.email && found_profile.name == matching_user.name
        }))
    }
}

mod find_with_opts {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, PartialEq, Eq, Debug)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
        index: khan::types::Int32,
    }

    #[async_test]
    async fn entity_with_default_options() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
            index: 0.into(),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_users =
            User::find_with_opts(get_mongo(), user::filter! { name: &name }, None, None, None)
                .await
                .unwrap();

        assert_eq!(found_users.len(), users.len());
        assert!(
            found_users.iter().all(|found_user| found_user
                == users.iter().find(|user| user.id == found_user.id).unwrap())
        )
    }

    #[async_test]
    async fn projection_with_default_options() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
            index: 0.into(),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_profiles = user::Profile::find_with_opts(
            get_mongo(),
            user::filter! { name: &name },
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(found_profiles.len(), users.len());
        assert!(found_profiles.iter().all(|found_profile| {
            let matching_user = users
                .iter()
                .find(|user| user.id == found_profile.id)
                .unwrap();
            found_profile.email == matching_user.email && found_profile.name == matching_user.name
        }))
    }

    #[async_test]
    async fn entity_with_custom_options() {
        let name = fakeit::name::full();

        let gen_user = |index: i32| User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
            index: index.into(),
        };

        let users = vec![
            gen_user(0),
            gen_user(1),
            gen_user(2),
            gen_user(3),
            gen_user(4),
        ];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_users = User::find_with_opts(
            get_mongo(),
            user::filter! { name: &name },
            Some(1),
            Some(3),
            Some({
                let mut order_by = indexmap::IndexMap::new();
                order_by.insert(user::Fields::Index, khan::Order::Asc);
                order_by
            }),
        )
        .await
        .unwrap();

        assert_eq!(found_users.len(), 3);
        assert_eq!(found_users[0], users[1]);
        assert_eq!(found_users[1], users[2]);
        assert_eq!(found_users[2], users[3]);
    }

    #[async_test]
    async fn projection_with_custom_options() {
        let name = fakeit::name::full();

        let gen_user = |index: i32| User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
            index: index.into(),
        };

        let users = vec![
            gen_user(0),
            gen_user(1),
            gen_user(2),
            gen_user(3),
            gen_user(4),
        ];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_profiles = user::Profile::find_with_opts(
            get_mongo(),
            user::filter! { name: &name },
            Some(1),
            Some(3),
            Some({
                let mut order_by = indexmap::IndexMap::new();
                order_by.insert(user::Fields::Index, khan::Order::Asc);
                order_by
            }),
        )
        .await
        .unwrap();

        assert_eq!(found_profiles.len(), 3);
        assert!(
            found_profiles[0].id == users[1].id
                && found_profiles[0].email == users[1].email
                && found_profiles[0].name == users[0].name
        );
        assert!(
            found_profiles[1].id == users[2].id
                && found_profiles[1].email == users[2].email
                && found_profiles[1].name == users[2].name
        );
        assert!(
            found_profiles[2].id == users[3].id
                && found_profiles[2].email == users[3].email
                && found_profiles[2].name == users[3].name
        );
    }
}

mod find_one {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, PartialEq, Eq, Debug)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn entity() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_user = User::find_one(get_mongo(), by_id(users[1].id))
            .await
            .unwrap();

        assert_eq!(found_user.as_ref(), Some(&users[1]));
    }

    #[async_test]
    async fn projection() {
        let name = fakeit::name::full();

        let gen_user = || User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: name.clone(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        let users = vec![gen_user(), gen_user(), gen_user()];

        User::insert_many(get_mongo(), &users).await.unwrap();

        let found_profile = user::Profile::find_one(get_mongo(), by_id(users[1].id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found_profile.id, users[1].id);
        assert_eq!(found_profile.email, users[1].email);
        assert_eq!(found_profile.name, users[1].name);
    }
}

mod find_one_and_update {
    use super::*;

    #[derive(Serialize, Deserialize, Entity, PartialEq, Eq, Debug)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn entity() {
        let user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let new_password = fakeit::password::generate(true, true, true, 16);

        let found = User::find_one_and_update(
            get_mongo(),
            by_id(user.id),
            user::update! { password: new_password.clone() },
        )
        .await
        .unwrap();

        assert_eq!(found.as_ref(), Some(&user));

        let updated = User::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.password, new_password);
    }

    #[async_test]
    async fn projection() {
        let user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let new_password = fakeit::password::generate(true, true, true, 16);

        let found = user::Profile::find_one_and_update(
            get_mongo(),
            by_id(user.id),
            user::update! { password: new_password.clone() },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);
        assert_eq!(found.name, user.name);

        let updated = User::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.password, new_password);
    }
}

mod patch {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn entity() {
        let mut user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let new_password = fakeit::password::generate(true, true, true, 16);

        user.patch(
            get_mongo(),
            user::update! { password: new_password.clone() },
        )
        .await
        .unwrap();

        assert_eq!(user.password, new_password);

        let found = User::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found.password, new_password);
    }

    #[async_test]
    async fn projection() {
        let user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let mut profile = user::Profile::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        let new_email = fakeit::contact::email();

        profile
            .patch(get_mongo(), user::update! { email: new_email.clone() })
            .await
            .unwrap();

        assert_eq!(profile.email, new_email);

        let found = user::Profile::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found.email, new_email);
    }
}

mod remove {
    use super::*;

    #[derive(Serialize, Deserialize, Entity)]
    #[entity(skip_schema_validation, projections(Profile(id, email, name)))]
    struct User {
        #[serde(rename = "_id")]
        id: ObjectId,
        email: String,
        #[serde(rename = "full_name")]
        name: String,
        password: String,
    }

    #[async_test]
    async fn entity() {
        let user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        user.remove(get_mongo()).await.unwrap();

        let found = User::find_one(get_mongo(), by_id(user.id)).await.unwrap();

        assert!(found.is_none());
    }

    #[async_test]
    async fn projection() {
        let user = User {
            id: ObjectId::new(),
            email: fakeit::contact::email(),
            name: fakeit::name::full(),
            password: fakeit::password::generate(true, true, true, 16),
        };

        user.insert(get_mongo()).await.unwrap();

        let profile = user::Profile::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap()
            .unwrap();

        profile.remove(get_mongo()).await.unwrap();

        let found = user::Profile::find_one(get_mongo(), by_id(user.id))
            .await
            .unwrap();

        assert!(found.is_none());
    }
}
