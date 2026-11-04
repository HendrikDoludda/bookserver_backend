# Loan window

State: blocked

**Blocked:** builds on the access model of the [download permission](24-download-permissions.md).

Optional: give a user access to a title until a date; after that the server stops authorizing
it. Enforced on the server, never by asking the app to stop.

## Build
- [ ] ? h · `expires_at` on the access row [DB]
- [ ] ? h · Stop authorizing a loaned title once `expires_at` has passed, checked on every request [BE]
- [ ] ? h · Set a loan's end date in the admin UI [FE]

## Open decisions
- [ ] What does the time limit apply to: access on the server (a loan), or a downloaded copy that stops working? — blocks: everything here. Suggested: the server-side loan. An expiring downloaded copy depends on the device enforcing its own restriction, which its owner controls — at most a cache-clearing convenience, never called enforcement.
