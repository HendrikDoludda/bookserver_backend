# Error responses & status codes

State: in progress

Every failure should reach the client as an honest status code and a JSON body
`{"error": "<key>"}` that the app translates through the [catalog](13-internationalization.md),
while the detail goes to the server log. `DatabaseError` already picks its status this way
(`EntryNotFound` → 404, anything else → 500 with the detail logged), but only 2 of the 10 error
enums turn into responses at all, and the auth handlers put raw error text into their 500 bodies.

## Build
- [x] 3 h · Give `AppErrors` an `IntoResponse` in `DatabaseError::into_response`'s shape — a status and a key per `AuthenticationError` / `EmailErrors` case (e.g. `EmailTaken` → 409, `IncorrectCredentials` → 401), the detail logged for 5xx and `{"error": "internal_error"}` sent instead — so handlers can use `?`. The specific errors only get this far once [26](26-database-layer-fixes.md) stops `setup_new_transaction` from flattening them [BE]
- [x] 1 h · `DatabaseError` answers with the same JSON body [BE]
- [x] 2 h · Remove `create_internal_server_error_response`, which puts the raw error text into every 500 body, once the handlers return `AppErrors` [BE]
- [x] 2 h · The extractors reject with `AppErrors`, and `RequestErrors` goes — its `RequestFailed` answers 400 for a failed database lookup too. Client faults become 401/403, server faults 500 [BE]
- [x] 1 h · `scan_all_directories` and `log_out` build their 500 bodies inline with `format!`, putting the error text in — log it and answer with a generic message [BE]
- [ ] 1.5 h · A body that can't be read (malformed JSON, a missing field, no `Content-Type`) is rejected by axum's own `Json` extractor with a plain-text 400/415/422 quoting the parse error — add our own `Json` extractor that rejects with `AppErrors` (400 `invalid_request`) and use it in every handler, before the app starts calling the API [BE]
- [ ] 1 h · The hollow handlers behind `PUT /update_entry`, `POST /insert_entry` and `POST /scan_metadata` answer 200 with "Failed to get response" (`failed_getting_response`) — answer 501 until they are built, like the TOTP stubs [BE]
- [ ] 1 h · Move the `log::` call sites to `tracing::` and drop the `log` dependency — consistency only; both already reach the output [BE]
- [ ] 0.5 h · Delete the error variants nothing constructs, with their rows in the error tests: `AuthenticationError::{AccountAlreadyExists, PasswordHashingFailure, ErrorCreatingSalt, UserNotFound}`, `EmailErrors::MissingEmail`, `DatabaseError::{EmailAlreadyExists, UsernameAlreadyExists}`, `ApplicationSetUpErrors::KeyEncodingFailure` [BE]
- [ ] 1.5 h · Every 429 sends `Retry-After` with the seconds left — login's throttle and the 2FA code lock ([04](04-two-factor-auth-totp.md)), which grows to 24 h — so the app can say when to try again; the error has to carry the time [BE]
