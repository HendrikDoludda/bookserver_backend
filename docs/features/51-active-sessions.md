# Active sessions

State: not started

A user sees where they are logged in — device, platform, last use — and can end any one of
those sessions. The `sessions` table already stores these columns.

## Build
- [ ] ? h · Endpoint listing the current user's sessions with device name, platform, and created and last-used times — never the token hashes [BE]
- [ ] ? h · Endpoint ending one of the current user's own sessions by id [BE]
- [ ] ? h · Active-sessions screen with an end-session button per device [FE]
