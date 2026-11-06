# Signed streaming URLs

State: blocked

**Blocked:** protects the restricted path of the [download permission](24-download-permissions.md),
which comes first.

Download-restricted users stream through short-lived signed URLs, so an address copied out of
the app stops working minutes later and can't be passed around — while a media player without
a session can still use it. A speed bump, not DRM.

## Build
- [ ] ? h · Issue signed streaming URLs: book id, user id and expiry, HMAC'd with `token_signing_key` from `secrets.toml` — the key is generated today but used by nothing, and isn't reachable from handlers (router state is `Arc<Database>` only) [BE]
- [ ] ? h · Check the signature and expiry before touching the file, accepting the current key and, until its expiry, `previous_token_signing_key` — otherwise every outstanding link breaks when the key rotates [BE]

## Open decisions
- [ ] How is the restricted path protected: (i) just never expose a download route or stable file URL, (ii) short-lived signed URLs, or (iii) encrypted delivery the app decrypts? — blocks: everything here. Suggested: (ii) — enforceable and cheap. (iii) needs a key exchange, key storage on the device and a decrypting player pipeline (a stock video player can't play an encrypted stream), and still ends at a screen recorder.
