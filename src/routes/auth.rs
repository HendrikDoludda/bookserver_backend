//set up
pub fn create_user() {
    //user submits an email, username and password (two times typed)
    //checks to see if the email/username already have an account
    //if not create user
    //send success message
    //show verify using email
    //send verification email
    //if yes send already has an account. or username already taken
}

pub fn verify_email() {
    //when user clicks on link from email it should route them to this call
    //here we will check if the link is valid.
    //the email verification table contains the expiration time as well as the code
    //if the token in inactive also fail the verification process
}

pub fn resend_verify_email() {
    //if called re-sends a verification email with a new token
}

fn send_verification_email() {
    //set previous token for user to inactive
    //create link
    //create email body
    //use the email_helper script to send an email
}

//authentication
pub fn log_in() {
    //user submits a password and username
    //retrieve stored password from db for user
    //using argon2 you verify the password if it matches the stored one
    //has a built in verification function which should be used
    //if not verified give the option to send verification email and stop here
    //otherwise continue
    //if totp is enabled first start the totp log in process and then create a new session
    //if successful you return the refresh token and the session token.
}

fn create_new_session() {
    //generate session token
    //generate refresh token
    //store tokens
    //return tokens
}

pub fn refresh_session_token() {
    //if refresh token is valid continue
    //generate new session token
    //generate new refresh token
    //update previous tokens with new ones
    //return new tokens to user
}

//modification
pub fn request_reset_password() {
    //create email body for resetting email
    //create a reset token
    //create link that redirects to reset password
    //attach to email body
    //send to user
    //if email is valid or invalid always display the same message
    //"If email is valid an email has been sent"
}

pub fn change_password() {
    //requires the old password and the same new password twice
    //if old password passes the argon verification continue
    //verify new password
    //hash and store the new password for the user
}

pub fn reset_password() {
    //requires the same new password twice and verify new password
    //if the token is valid continue
    //remove reset token
    //store new password for the user.
}

fn verify_new_password() {
    //requires the same new password twice
    //if the new passwords are the same continue
    //if the new password is not the same as the old password continue
}

fn password_changed() {
    //remove all active sessions and refresh tokens.
}

//sign out
pub fn log_out() {
    //delete the session and refresh token entry to inactive
    //return success
}
