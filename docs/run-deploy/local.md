# Local

## Commands
| Key | Command |
|---|---|
| `run` | `BOOK_DIRS=/path/to/books cargo run` |
| `mailpit` | `brew install mailpit && mailpit` |
| `run-mail` | `BOOK_DIRS=/path/to/books SMTP_HOST=localhost SMTP_PORT=1025 SMTP_TLS=none SMTP_FROM=server@localhost SMTP_FROM_NAME="Book Server" cargo run` |
| `health` | `curl http://localhost:8080/health` |
| `sign-up` | `curl -X POST http://localhost:8080/api/v1/auth/register -H 'Content-Type: application/json' -d '{"email":"PASTE_EMAIL","username":"PASTE_USERNAME","password":"PASTE_PASSWORD","password_repeat":"PASTE_PASSWORD","device_name":"terminal","platform":"macos","setup_code":"PASTE_SETUP_CODE"}' -w '\n'` |
| `login` | `curl -X POST http://localhost:8080/api/v1/auth/login -H 'Content-Type: application/json' -d '{"username_or_email":"PASTE_USERNAME","password":"PASTE_PASSWORD","device_name":"terminal","platform":"macos"}' -w '\n'` |
| `scan` | `curl -X POST http://localhost:8080/api/v1/scan -H 'Authorization: Bearer PASTE_SESSION_TOKEN' -w '\n'` |

## Run
1. For PDF covers only: put the PDFium shared library (`libpdfium.dylib`, `libpdfium.so` or `pdfium.dll`, prebuilt at [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)) in the working directory or on the library path.
2. Start the server — [`run`](#commands). `BOOK_DIRS` is `:`-separated and `PORT` defaults to 8080. Email is off unless `SMTP_HOST` is set; then `SMTP_PORT`, `SMTP_FROM` and `SMTP_FROM_NAME` are required, `SMTP_USERNAME` and `SMTP_PASSWORD` are optional but go together, and `SMTP_TLS` is `starttls` (default), `implicit` or `none`. An incomplete mail setup stops the server at start. The first start creates `data/databases/app_data.sqlite`, and `secrets.toml` in the OS config directory (on macOS `~/Library/Application Support/com.placeholder_organization.bookserver_backend/`).
3. To read the server's mail while testing, start Mailpit in a second terminal — [`mailpit`](#commands) — and start the server with [`run-mail`](#commands) instead. Every mail shows up at http://localhost:8025.
4. Check that it answers — [`health`](#commands).
5. Sign up the first account — [`sign-up`](#commands), with `PASTE_SETUP_CODE` from the server's start log. It becomes the admin; its answer's `session_token` is good for 15 minutes, and [`login`](#commands) gives a new one.
6. Catalog the book folders — [`scan`](#commands), with the admin's `session_token`.
