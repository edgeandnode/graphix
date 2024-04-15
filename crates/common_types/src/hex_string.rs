use core::fmt;
use std::fmt::{Debug, Display};
use std::str::FromStr;

use diesel::backend::Backend;
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::ToSql;
use diesel::sql_types;
use hex::FromHex;
use quickcheck::Arbitrary;
use serde::{Deserialize, Serialize};

/// A [`serde`], [`diesel`], and [`async_graphql`]-compatible wrapper around a
/// hex-encoded byte sequence (of arbitrary length) with `0x` prefix. Parsing
/// and deserializing from hex strings without the `0x` prefix is also allowed.
///
/// You should generally try to avoid using this type directly, and instead
/// alias it to something more descriptive for its intended use case, possibly
/// by enforcing a specific length.
#[derive(
    Copy,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    AsExpression,
    FromSqlRow,
    derive_more::From,
)]
// TODO: The fact that we SQL-encode all kinds of hex strings, even fixed-length
// ones, as variable-length byte sequences is a bit of a wart. Not that big of a
// deal though.
#[diesel(sql_type = sql_types::Binary)]
pub struct HexString<T>(pub T);

impl<T: ToOwned> HexString<T> {
    pub fn owned(&self) -> HexString<T::Owned>
    where
        T: ToOwned,
    {
        HexString(self.0.to_owned())
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
        // The `0x` prefix is optional.
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

impl<T> ToSql<sql_types::Binary, Pg> for HexString<T>
where
    T: AsRef<[u8]> + Debug,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        ToSql::<sql_types::Binary, Pg>::to_sql(self.0.as_ref(), out)
    }
}

impl<T> FromSql<sql_types::Binary, Pg> for HexString<T>
where
    T: TryFrom<Vec<u8>>,
    T::Error: Debug,
{
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        T::try_from(FromSql::<sql_types::Binary, Pg>::from_sql(bytes)?)
            .map(HexString)
            .map_err(|e| anyhow::anyhow!("{:?}", e).into())
    }
}

impl<T: Arbitrary> Arbitrary for HexString<T> {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        Self(T::arbitrary(g))
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::ScalarType;
    use quickcheck_macros::quickcheck;

    use super::*;

    #[quickcheck]
    fn async_graphql_roundtrip(hex_string: HexString<Vec<u8>>) -> bool {
        let async_graphql_value = hex_string.to_value();
        let hex_string2: HexString<Vec<u8>> =
            async_graphql::ScalarType::parse(async_graphql_value).unwrap();

        hex_string == hex_string2
    }

    #[quickcheck]
    fn serde_roundtrip(hex_string: HexString<Vec<u8>>) -> bool {
        let json = serde_json::to_string(&hex_string).unwrap();
        let hex_string2: HexString<Vec<u8>> = serde_json::from_str(&json).unwrap();

        hex_string == hex_string2
    }

    #[quickcheck]
    fn always_starts_with_0x(hex_string: HexString<Vec<u8>>) -> bool {
        hex_string.to_string().starts_with("0x")
    }

    #[test]
    fn decodable_without_0x() {
        let hex_string: HexString<Vec<u8>> = "deadbeef".parse().unwrap();
        assert_eq!(hex_string.to_string(), "0xdeadbeef");
    }

    #[quickcheck]
    fn from_str_roundtrip(hex_string: HexString<Vec<u8>>) -> bool {
        let string = hex_string.to_string();
        let hex_string2: HexString<Vec<u8>> = string.parse().unwrap();

        hex_string == hex_string2
    }
}
