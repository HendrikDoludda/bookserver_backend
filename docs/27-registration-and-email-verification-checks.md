# Registration & email verification — checks

On a freshly wiped database ([`commands.md`](commands.md), destructive), with the server started
through Mailpit ([`run-mail`](run-deploy/local.md#commands)). Every request is a `POST` with
`Content-Type: application/json`.

- [ ] The server log shows `No account exists yet: the first sign-up needs the setup code …`
- [ ] `/api/v1/auth/register` with `{"email":"admin@example.com","username":"admin","password":"correct horse battery","password_repeat":"correct horse battery","device_name":"mac","platform":"macos"}` → 403 `invalid_setup_code`; the same with `"setup_code":"<the code>"` added → 200 with tokens, and no mail arrives in Mailpit
- [ ] The same for `bob` / `bob@example.com`, no setup code needed → 202 `{"verification_required":true}`, and one mail with an 8-character code arrives
- [ ] `/api/v1/auth/login` as `bob` → 403 `email_not_verified`
- [ ] `/api/v1/auth/verify_email` with `{"username_or_email":"bob","password":"correct horse battery","code":"<the code>","device_name":"mac","platform":"macos"}` → 200 with tokens
- [ ] As `bob`: `/api/v1/auth/login` → 200; `/api/v1/auth/refresh` with the refresh token as `Authorization: Bearer` → 200; `/api/v1/auth/logout` → 204
