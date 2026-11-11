use serde::{Deserialize, Serialize};

use crate::models::UserSettings;

/// `GET` and `PUT /api/v1/account/settings`: the user's settings, without the user id. `None` is a
/// setting the user hasn't chosen, and a field left out of a `PUT` is one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub locale: Option<String>,
}

impl From<UserSettings> for Settings {
    fn from(row: UserSettings) -> Self {
        Settings { locale: row.locale }
    }
}
