use diesel::{Insertable, Queryable, Selectable};

use crate::schema::blacklisted_tokens;

#[derive(Clone, Debug, Queryable, Selectable)]
#[allow(dead_code)]
#[diesel(table_name = blacklisted_tokens)]
pub struct BlacklistedToken {
    pub token_signature_hex: String,
    pub token_expiration: i64,
}

#[derive(Insertable)]
#[diesel(table_name = blacklisted_tokens)]
pub struct NewBlacklistedToken<'a> {
    pub token_signature_hex: &'a str,
    pub token_expiration: i64,
}
