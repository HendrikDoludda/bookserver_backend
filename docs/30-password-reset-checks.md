# Password reset — checks

With Mailpit, on `bob` from the [registration checks](27-registration-and-email-verification-checks.md).

- [ ] `POST /api/v1/auth/password_reset` with `{"email":"bob@example.com"}` → 202, and one mail with a code arrives in Mailpit
- [ ] `POST /api/v1/auth/password_reset/confirm` with `{"email":"bob@example.com","code":"<the code>","new_password":"new horse battery 2","new_password_repeat":"new horse battery 2"}` → 200
- [ ] Login with the old password → 401, with the new one → 200; a session token from before the reset → 401 on `PUT /api/v1/update_entry`
