use core::fmt;
use std::fmt::{Debug, Display};
use std::str::FromStr;

use diesel::backend::Backend;
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::ToSql;
use diesel::sql_types;
use hex::FromHex;
use serde::{Deserialize, Serialize};

/// A [`serde`], [`diesel`], and [`async_graphql`]-compatible wrapper around a
/// hex-encoded byte sequence (of arbitrary length) with `0x` prefix. Parsing
/// and deserializing from hex strings without the `0x` prefix is also allowed.
///
/// You should generally try to avoid using this type directly, and instead
/// alias it to something more descriptive for its intended use case, possibly
/// by enforcing a specific length.
#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, AsExpression, FromSqlRow,
)]
// TODO: The fact that we SQL-encode all kinds of hex strings, even fixed-length
// ones, as variable-length byte sequences is a bit of a wart. Not that big of a
// deal, but maybe there's a better way to do this.
#[diesel(sql_type = ::diesel::sql_types::Binary)]
pub struct HexString<T>(pub T);

impl<T: ToOwned> HexString<T> {
    pub fn owned(&self) -> HexString<T::Owned>
    where
        T: ToOwned,
    {
        HexString(self.0.to_owned())
    }
}

impl<T> From<T> for HexString<T> {
    fn from(t: T) -> Self {
        HexString(t)
    }
}

#[async_graphql::Scalar]
impl<T> async_graphql::ScalarType for HexString<T>
where
    T: AsRef<[u8]> + FromHex + Send + Sync,
{
    fn parse(value: async_graphql::Value) -> async_graphql::InputValueResult<Self> {
        Ok(Deserialize::deserialize(value.into_json()?)?)
    }

    fn to_value(&self) -> async_graphql::Value {
        async_graphql::Value::String(self.to_string())
    }
}

impl<T: AsRef<[u8]>> Display for HexString<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0.as_ref()))
    }
}

impl<T: AsRef<[u8]>> Serialize for HexString<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self)
    }
}

impl<T: FromHex> FromStr for HexString<T> {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let stripped = s.strip_prefix("0x").unwrap_or(s);
        FromHex::from_hex(stripped)
            .map(Self)
            .map_err(|_| "invalid hex string")
    }
}

impl<'a, T: FromHex> Deserialize<'a> for HexString<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl<T> schemars::JsonSchema for HexString<T> {
    fn schema_name() -> String {
        "HexString".to_owned()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        gen.subschema_for::<String>()
    }
}

impl<T, Db> ToSql<sql_types::Binary, Db> for HexString<T>
where
    T: AsRef<[u8]> + Debug,
    Db: Backend,
    [u8]: ToSql<sql_types::Binary, Db>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Db>,
    ) -> diesel::serialize::Result {
        ToSql::<sql_types::Binary, Db>::to_sql(self.0.as_ref(), out)
    }
}

impl<T, Db> FromSql<sql_types::Binary, Db> for HexString<T>
where
    T: TryFrom<Vec<u8>>,
    T::Error: Debug,
    Db: Backend,
    Vec<u8>: FromSql<sql_types::Binary, Db>,
{
    fn from_sql(bytes: <Db as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        T::try_from(Vec::from_sql(bytes)?)
            .map(HexString)
            .map_err(|e| anyhow::anyhow!("{:?}", e).into())
    }
}
