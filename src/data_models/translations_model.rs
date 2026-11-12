use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One language in `GET /api/v1/locales`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Locale {
    pub code: String,
    /// The language's name in itself: `Deutsch`, not `German`.
    pub display_name: String,
    pub icon: Option<String>,
    pub is_default: bool,
}

/// The query of `GET /api/v1/locales/{code}/translations`.
#[derive(Debug, Deserialize)]
pub struct TranslationsQuery {
    /// Namespaces, comma-separated; absent or empty means all of them.
    pub sections: Option<String>,
}

/// `GET /api/v1/locales/{code}/translations`: by namespace, then by name, the text.
pub type Translations = BTreeMap<String, BTreeMap<String, String>>;
