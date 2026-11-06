# Local

## Commands
| Key | Command |
|---|---|
| `run` | `BOOK_DIRS=/path/to/books cargo run` |
| `health` | `curl http://localhost:8080/health` |
| `scan` | `curl -X POST http://localhost:8080/api/v1/scan_all_directories` |

## Run
1. For PDF covers only: put the PDFium shared library (`libpdfium.dylib`, `libpdfium.so` or `pdfium.dll`, prebuilt at [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)) in the working directory or on the library path.
2. Start the server — [`run`](#commands). `BOOK_DIRS` is `:`-separated; `PORT` (default 8080) and the `SMTP_HOST` / `SMTP_PORT` / `SMTP_USERNAME` / `SMTP_PASSWORD` / `SMTP_FROM` / `SMTP_FROM_NAME` mail settings are optional. The first start creates `data/databases/app_data.sqlite`, and `secrets.toml` in the OS config directory (on macOS `~/Library/Application Support/com.placeholder_organization.bookserver_backend/`).
3. Check that it answers — [`health`](#commands).
4. Catalog the book folders — [`scan`](#commands).
