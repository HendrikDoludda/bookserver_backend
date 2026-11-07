//! The state every handler can reach. A handler asks for just the part it needs —
//! `State<Arc<Database>>` or `State<Mailer>` — and the `FromRef` impls hand it over.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::{db::Database, routes::email_helper::Mailer};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub mailer: Mailer,
}

impl FromRef<AppState> for Arc<Database> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.db)
    }
}

impl FromRef<AppState> for Mailer {
    fn from_ref(state: &AppState) -> Self {
        state.mailer.clone()
    }
}
