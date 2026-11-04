# Error responses & status codes

State: in progress

Every failure should reach the client as an honest status code with a generic body, while
the detail goes to the server log. `DatabaseError` already works this way (`EntryNotFound` →
404, anything else → 500 with the detail logged), but only 2 of the 10 error enums turn into
responses at all, and the auth handlers put raw error text into their 500 bodies.

## Build
- [ ] ? h · Give `AppErrors` an `IntoResponse` in `DatabaseError::into_response`'s shape — a status per `AuthenticationError` / `EmailErrors` case (e.g. `EmailTaken` → 409, `IncorrectCredentials` → 401), the detail logged for 5xx, a generic body for the client — so handlers can use `?`. The specific errors only get this far once [26](26-database-layer-fixes.md) stops `setup_new_transaction` from flattening them [BE]
- [ ] ? h · Remove `create_internal_server_error_response`, which puts the raw error text into every 500 body, once the handlers return `AppErrors` [BE]
- [ ] ? h · Split `RequestErrors::RequestFailed` per the request-failed decision — the three `// 500` comments next to it in the extractors say the opposite of what it does today [BE]
- [ ] ? h · `scan_all_directories` and `log_out` build their 500 bodies inline with `format!`, putting the error text in — log it and answer with a generic message [BE]
- [ ] ? h · The hollow handlers behind `PUT /update_entry`, `POST /insert_entry` and `POST /scan_metadata` answer 200 with "Failed to get response" (`failed_getting_response`) — answer 501 until they are built, like the TOTP stubs [BE]
- [ ] ? h · Move the `log::` call sites to `tracing::` and drop the `log` dependency — consistency only; both already reach the output [BE]

## Open decisions
- [ ] `RequestErrors::RequestFailed` covers both a malformed request and a failed database lookup — keep it 400, or split it into a client-fault (400) and a server-fault (500) variant? — blocks: splitting `RequestFailed`. Suggested: split — the two need different statuses, and a real 5xx case is what makes the logging branch in its `IntoResponse` worth having (today that branch is dead code).
