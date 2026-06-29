//set up
pub fn begin_totp_setup() {
    // Ensure user is authenticated.
    // Generate a random TOTP secret.
    // Store the secret as "pending" (not yet enabled).
    // Create the otpauth:// URI.
    // Return the URI (or QR code) and the manual setup key.
}

pub fn confirm_totp_setup() {
    // User submits a 6-digit code from their authenticator app.
    // Verify the code using the pending secret.
    // If valid:
    //     Mark TOTP as enabled.
    //     Move pending secret to active.
    //     return Generate recovery codes.
}

fn generate_recovery_codes() {
    //don't know what is required for this call
    //generate recovery codes (10)
    //this only happens when user does not have recovery codes.
    //will be displayed only once
    //store hashed codes
    //returns recovery codes
}

//verify
pub fn verify_totp_log_in() {
    //user sends in the session code
    //verify code
    //when validated create session and refresh token
    //send to user
}

pub fn authenticate_using_recovery_code() {
    //user uses recovery code
    //if recovery code has been used return error
    //checks against the hashed recovery code in the db
    //if check is true then log in
    //mark code as used
}

//Disable
pub fn disable_totp() {
    // Require current password (or another strong verification).
    // Delete TOTP secret.
    // Delete all recovery codes.
    //invalidate all active sessions (refresh tokens, session tokens,recovery tokens)
    // Mark TOTP as disabled.
}
