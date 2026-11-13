# Active sessions

State: not started

A user sees where they are logged in — device, platform, last use — and can end any one of
those sessions. The `sessions` table already stores these columns.

## Build
- [ ] ? h · Endpoint listing the current user's sessions with device name, platform, and created and last-used times — never the token hashes, and only sessions whose refresh token hasn't run out (`refresh_token_valid_until > now`): a login removes the others only at the user's next login [BE]
- [ ] ? h · Endpoint ending one of the current user's own sessions by id [BE]
- [ ] ? h · Active-sessions screen with an end-session button per device [FE]
- [ ] ? h · The app sends the device's own name as `device_name`, for example through `device_info_plus` — until then it sends the OS name [FE]
