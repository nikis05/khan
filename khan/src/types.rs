use mongodb::bson;
use schemars::JsonSchema;
use schemars::schema::SchemaObject;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::ops::{Deref, DerefMut};
use std::{borrow::Borrow, fmt::Display};

macro_rules! impl_wrapper {
    ($outer:ty, $inner:ty, $bson_type:literal) => {
        impl AsRef<$inner> for $outer {
            fn as_ref(&self) -> &$inner {
                &self.0
            }
        }

        impl AsMut<$inner> for $outer {
            fn as_mut(&mut self) -> &mut $inner {
                &mut self.0
            }
        }

        impl Borrow<$inner> for $outer {
            fn borrow(&self) -> &$inner {
                &self.0
            }
        }

        impl BorrowMut<$inner> for $outer {
            fn borrow_mut(&mut self) -> &mut $inner {
                &mut self.0
            }
        }

        impl Deref for $outer {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $outer {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl From<$inner> for $outer {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl From<$outer> for $inner {
            fn from(value: $outer) -> Self {
                value.0
            }
        }

        impl JsonSchema for $outer {
            fn schema_name() -> String {
                std::stringify!($outer).into()
            }

            fn json_schema(
                _gen: &mut schemars::r#gen::SchemaGenerator,
            ) -> schemars::schema::Schema {
                SchemaObject {
                    extensions: {
                        let mut extensions = schemars::Map::new();
                        extensions.insert("bsonType".into(), $bson_type.into());
                        extensions
                    },
                    ..Default::default()
                }
                .into()
            }
        }
    };
}

macro_rules! forward_display {
    ($ty:ty) => {
        impl Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct ObjectId(pub mongodb::bson::oid::ObjectId);

forward_display!(ObjectId);

impl_wrapper!(ObjectId, mongodb::bson::oid::ObjectId, "objectId");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Regex(pub bson::Regex);

forward_display!(Regex);

impl_wrapper!(Regex, bson::Regex, "regex");

#[derive(Clone, Debug, Default, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct JavaScriptCode(pub String);

forward_display!(JavaScriptCode);

impl_wrapper!(JavaScriptCode, String, "javascript");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct JavaScriptCodeWithScope(pub bson::JavaScriptCodeWithScope);

forward_display!(JavaScriptCodeWithScope);

impl_wrapper!(
    JavaScriptCodeWithScope,
    bson::JavaScriptCodeWithScope,
    "javascript"
);

#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct Int32(pub i32);

forward_display!(Int32);

impl_wrapper!(Int32, i32, "int");

#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct Int64(pub i64);

forward_display!(Int64);

impl_wrapper!(Int64, i64, "long");

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Timestamp(pub bson::Timestamp);

forward_display!(Timestamp);

impl_wrapper!(Timestamp, bson::Timestamp, "timestamp");

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(transparent)]
pub struct Binary(pub bson::Binary);

forward_display!(Binary);

impl_wrapper!(Binary, bson::Binary, "binData");

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct DateTime(pub bson::DateTime);

forward_display!(DateTime);

impl_wrapper!(DateTime, bson::DateTime, "date");

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(transparent)]
pub struct Decimal128(pub bson::Decimal128);

forward_display!(Decimal128);

impl_wrapper!(Decimal128, bson::Decimal128, "decimal");
