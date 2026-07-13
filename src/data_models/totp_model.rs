pub struct TOTPSignInRequest {
    pub code: String,
}

pub struct RecoveryCodeSignInRequest {
    pub code: String,
}

pub struct DisableTOTPRequest {
    pub password: String,
}
