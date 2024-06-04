use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::ToSql;
use diesel::sql_types;

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsExpression,
    FromSqlRow,
    async_graphql::Enum,
)]
#[diesel(sql_type = sql_types::Integer)]
pub enum ApiKeyPermissionLevel {
    Admin,
}

impl ToSql<sql_types::Integer, Pg> for ApiKeyPermissionLevel {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        match self {
            ApiKeyPermissionLevel::Admin => <i32 as ToSql<sql_types::Integer, Pg>>::to_sql(&1, out),
        }
    }
}

impl FromSql<sql_types::Integer, Pg> for ApiKeyPermissionLevel {
    fn from_sql(bytes: PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            1 => Ok(ApiKeyPermissionLevel::Admin),
            _ => Err(anyhow::anyhow!("invalid permission level").into()),
        }
    }
}
