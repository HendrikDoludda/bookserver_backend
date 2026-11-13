# Account screens

State: in progress

The app's screens for the account flows the server offers — login, two-factor auth and recovery
codes, sign-up and email verification, password reset, and the language setting — built on the
[app skeleton](73-flutter-app.md).

## Build
- [ ] 3 h · Login screen: username or email plus password [FE]
- [ ] 1 h · Log-out action [FE]
- [ ] 2 h · Code-entry step after the password, when login answers `totp_required`; it replaces the `login.totp_unavailable` notice, whose text is deleted [FE]
- [ ] 1 h · "Use a recovery code" option on the code-entry step [FE]
- [ ] 4 h · 2FA settings: enable (QR code + confirm) and disable [FE]
- [ ] 1 h · Show the recovery codes once, right after 2FA is switched on [FE]
- [ ] 3 h · Sign-up screen [FE]
- [ ] 2 h · Verification-code screen, with "send a new code"; it asks for the password again when the app no longer holds it [FE]
- [ ] 2 h · "Forgot password" form [FE]
- [ ] 2 h · Code and new-password screen [FE]
- [ ] 2 h · Settings screen with a language picker [FE]
