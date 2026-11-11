use serde::{Deserialize, Serialize};

use crate::models::Role;

/// One account as an admin sees it — never the password hash. Times are unix seconds.
#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub role: Role,
    pub created_at: i64,
    pub last_login: Option<i64>,
}

/// `PATCH /api/v1/users/{id}`: a field that is left out stays as it is.
#[derive(Debug, Deserialize)]
pub struct UserChanges {
    pub role: Option<Role>,
    pub email_verified: Option<bool>,
}
