# Building signed releases

Update artifacts are signed with an Ed25519 keypair. The private key is
**never committed**. Set these env vars before building:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -Raw src-tauri\.tauri-signing.key)
# If the key has a password:
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<password>"
npm run tauri:build
```

`npm run tauri:build` produces:

- `src-tauri/target/release/bundle/msi/*.msi`
- `src-tauri/target/release/bundle/msi/latest.json`

Upload the MSI + `latest.json` to the GitHub Release:

```bash
gh release create v0.1.0 \
  src-tauri/target/release/bundle/msi/*.msi \
  src-tauri/target/release/bundle/msi/latest.json \
  --repo thichcode/remotemanager --title "v0.1.0"
```

The updater endpoint is `https://github.com/thichcode/remotemanager/releases/latest/download/latest.json`, so the release assets must be named exactly `latest.json` and `Remote Manager_0.1.0_x64_en-US.msi` (or the plain `*.msi` file).
