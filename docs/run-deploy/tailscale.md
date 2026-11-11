# Tailscale

## Commands
| Key | Command |
|---|---|
| `cli-alias` | `alias tailscale=/Applications/Tailscale.app/Contents/MacOS/Tailscale` |
| `run` | `BIND_ADDRESS=127.0.0.1 BOOK_DIRS=/path/to/books cargo run` |
| `serve` | `tailscale serve --bg 8080` |
| `serve-off` | `tailscale serve --https=443 off` |

## Run
1. Install Tailscale on the server machine and sign in. On macOS, Tailscale recommends the standalone app from [tailscale.com/download](https://tailscale.com/download), which installs the `tailscale` command from its settings (CLI integration); with the App Store app, make the command available in the terminal — [`cli-alias`](#commands).
2. In the Tailscale admin console, under DNS, turn on MagicDNS and HTTPS Certificates.
3. Start the server for this machine only — [`run`](#commands).
4. Let Tailscale answer HTTPS on the machine's tailnet name and forward to the server — [`serve`](#commands). It prints the `https://<machine>.<tailnet>.ts.net` URL every tailnet device reaches the server at, keeps serving in the background, and renews the certificate itself; with another `PORT`, serve that port.
5. To stop serving — [`serve-off`](#commands).
