use std::str::FromStr;

use diesel::backend::Backend;
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::ToSql;
use diesel::sql_types;
use quickcheck::Arbitrary;
use serde::{Deserialize, Serialize};

/// A [`serde`], [`diesel`], and [`async_graphql`]-compatible type definition
/// for IPFS CIDs and subgraph deployment IDs.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    AsExpression,
    FromSqlRow,
    derive_more::Display,
    derive_more::FromStr,
)]
#[diesel(sql_type = sql_types::Text)]
pub struct IpfsCid(cid::Cid);

#[async_graphql::Scalar]
impl async_graphql::ScalarType for IpfsCid {
    fn parse(value: async_graphql::Value) -> async_graphql::InputValueResult<Self> {
        let async_graphql::Value::String(string) = value else {
            return Err(async_graphql::InputValueError::expected_type(value));
        };

        let cid = cid::Cid::from_str(&string)?;
        Ok(IpfsCid(cid))
    }

    fn to_value(&self) -> async_graphql::Value {
        async_graphql::Value::String(self.0.to_string())
    }
}

impl ToSql<sql_types::Text, Pg> for IpfsCid {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        ToSql::<sql_types::Text, Pg>::to_sql(&self.to_string(), &mut out.reborrow())
    }
}

impl FromSql<sql_types::Text, Pg> for IpfsCid {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let s = String::from_sql(bytes)?;
        Ok(IpfsCid(cid::Cid::from_str(&s)?))
    }
}

impl Arbitrary for IpfsCid {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        Self(cid::Cid::arbitrary(g))
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::ScalarType;
    use quickcheck_macros::quickcheck;

    use super::*;

    #[quickcheck]
    fn async_graphql_roundtrip(ipfs_cid: IpfsCid) -> bool {
        let async_graphql_value = ipfs_cid.to_value();
        let ipfs_cid2: IpfsCid = ScalarType::parse(async_graphql_value).unwrap();

        ipfs_cid == ipfs_cid2
    }

    #[quickcheck]
    fn serde_roundtrip(ipfs_cid: IpfsCid) -> bool {
        let json = serde_json::to_string(&ipfs_cid).unwrap();
        let ipfs_cid2: IpfsCid = serde_json::from_str(&json).unwrap();

        ipfs_cid == ipfs_cid2
    }

    #[quickcheck]
    fn from_str_roundtrip_quickcheck(ipfs_cid: IpfsCid) -> bool {
        let string = ipfs_cid.to_string();
        let ipfs_cid2 = IpfsCid::from_str(&string).unwrap();

        ipfs_cid == ipfs_cid2
    }

    #[test]
    fn from_str_roundtrip() {
        let deployment_id = "QmNY7gDNXHECV8SXoEY7hbfg4BX1aDMxTBDiFuG4huaSGA";
        let ipfs_id = IpfsCid::from_str(deployment_id).unwrap();

        assert_eq!(ipfs_id.to_string(), deployment_id);
    }
}
