//! The state every handler can reach. A handler asks for just the part it needs —
//! `State<Arc<Database>>`, `State<Mailer>`, `State<SetupCode>` or `State<TotpCipher>` — and the
//! `FromRef` impls hand it over.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::{
    db::Database,
    routes::{email_helper::Mailer, registration::SetupCode, totp_cipher::TotpCipher},
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub mailer: Mailer,
    pub setup_code: SetupCode,
    pub totp_cipher: TotpCipher,
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

impl FromRef<AppState> for SetupCode {
    fn from_ref(state: &AppState) -> Self {
        state.setup_code
    }
}

impl FromRef<AppState> for TotpCipher {
    fn from_ref(state: &AppState) -> Self {
        state.totp_cipher.clone()
    }
}
