# Public-exposure hardening

State: not started

Hardening that only matters if the server is ever reachable from the open internet rather than
only the LAN and Tailscale. Parked until then. Throttling login per account is not parked: any
device on the LAN can reach login, so it lives in [03](03-login-and-sessions.md).

## Build
- [ ] ? h · Rate-limit requests per client IP (e.g. `tower_governor`, a new dependency) [BE]
