# Bugfixes

State: not started

Bugs that belong to no feature. They get fixed whenever it fits — this list never holds up a
tier.

## Build
- [ ] 0.5 h · Remove `POST /api/v1/setup_db` and `initiate_database` — the migrations are compiled in and run at every start, so the endpoint never has anything to do, and its `Database::new()` builds and drops a second connection pool. Drop its mentions in `project-state.md`, the README's API table and [28](28-roles-and-access-control.md) [BE]
- [ ] ? h · Wire up or delete `config::get_api_key`, per the API-key decision — it reads `API_KEY`, but nothing calls it, so the env var does nothing [BE]

## Open decisions
- [ ] What is `API_KEY` for? — blocks: wiring up or deleting `get_api_key`. Suggested: delete it — authentication runs on sessions ([03](03-login-and-sessions.md)).
