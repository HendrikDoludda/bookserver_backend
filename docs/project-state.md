# Project state

Not released yet. What works when the server runs today — every endpoint except the health check,
the auth routes and the translation catalog needs a logged-in user:

- **Scanning** the `BOOK_DIRS` folders (`POST /api/v1/scan`, admin only): PDF, EPUB, CBZ and image-folder books become book, series and library rows; volume, chapter and page come from the file names; one cover per book is extracted. Only lowercase extensions are recognised, and PDFs need the PDFium library — without it they are left out.
- **Streaming** a book file with HTTP range requests (`GET /api/v1/books/{id}/file`).
- **Browsing**: the libraries (`GET /api/v1/libraries`), a library's series (`GET /api/v1/libraries/{id}/series`), a series' books (`GET /api/v1/series/{id}/books`) and one book's metadata (`GET /api/v1/books/{id}`).
- **Deleting a library** (`DELETE /api/v1/libraries/{id}`, admin only) — its series and books stay.
- **Sign-up** with email verification (`POST /api/v1/auth/register`, `/auth/verify_email`, `/auth/verify_email/resend`): the first account needs the setup code from the server's start log and becomes the admin; without email set up, an address counts as verified at once; an account left unverified for 48 hours frees its name and address for the next sign-up.
- **Login and sessions** with username or email (`POST /api/v1/auth/login`, `/auth/refresh`, `/auth/logout`): a 15-minute session token and a rotating 30-day refresh token; repeated wrong passwords are throttled.
- **Two-factor auth** with an authenticator app (`/api/v1/account/totp…`, `/auth/login/totp`, `/auth/login/recovery_code`): ten single-use recovery codes that can be renewed, and a lock on wrong codes.
- **Password reset** by emailed code (`POST /api/v1/auth/password_reset`, `/auth/password_reset/confirm`): it also confirms the email address and ends every session.
- **Account changes** (`/api/v1/account/password`, `/account/username`, `/account/email`, `/account/email/confirm`): the password (the other sessions end), the username, and the email — confirmed by a code sent to the new address, or at once without email.
- **Language setting** per user (`GET` and `PUT /api/v1/account/settings`, or chosen at sign-up): one of the languages the server offers. The code mails — verification, password reset, email change — are written in it, else in the default language, English.
- **Translation catalog** in English and German (`GET /api/v1/locales`, `GET /api/v1/locales/{code}/translations`): the apps' texts by namespace, English wherever a language lacks one, sent with an ETag so an app re-checks its copy without downloading it again. The texts ship in `translations/` and are synced into the database at every start.
- **User management** (`GET /api/v1/users`, `PATCH /api/v1/users/{id}`, admin only): every account, its role, and marking an email verified; the last admin stays an admin.
- **HTTPS** through `tailscale serve` ([tailscale.md](run-deploy/tailscale.md)), with `BIND_ADDRESS` and `PORT` choosing where the server listens.
- **Health check** (`GET /health`).

Next milestone: the Flutter app ([73](features/73-flutter-app.md)), built on this catalog ([57](features/57-frontend-localization.md)) — see the [checklist](build-checklist.md).
