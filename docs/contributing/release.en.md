# Publishing a release

End-to-end process for shipping a new RL Stats Overlay version on GitHub Releases with **automatic in-app updates** for existing users.

## Prerequisites (one-shot)

To do **once** per maintainer, before the very first signed release.

### 1. Generate the Tauri keypair

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.tauri-keys" | Out-Null
bun tauri signer generate -w "$env:USERPROFILE\.tauri-keys\rl-stats-overlay.key"
```

The command asks for a **password** (leave blank if you don't want one, but it's less safe). Note it down, **you'll need it for CI**.

Two files are created:
- `~/.tauri-keys/rl-stats-overlay.key` — **private** key (NEVER commit, NEVER publish)
- `~/.tauri-keys/rl-stats-overlay.key.pub` — **public** key (already in `tauri.conf.json:pubkey`)

> 💾 **Backup mandatory**: copy the private `.key` to an external drive / encrypted cloud. If you lose it, the update chain is broken forever — users will have to manually reinstall to switch to a new key.

### 2. Store the secrets in GitHub

In the repo: **Settings → Secrets and variables → Actions → New repository secret**:

| Secret name | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | The **full content** of the `.key` file (base64). Grab it with `Get-Content ~/.tauri-keys/rl-stats-overlay.key | Set-Clipboard`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The password you typed during `signer generate` (leave empty if you didn't set one). |

## Release process (every version)

### 1. Bump the version

Three files to update with the **same number**:

- `package.json` → `"version"` field
- `src-tauri/Cargo.toml` → `version` field under `[package]`
- `src-tauri/tauri.conf.json` → `"version"` field

```powershell
# Example for 0.1.7 — edit the 3 files manually, or use a script.
```

### 2. Commit + tag + push

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.1.7"
git tag v0.1.7
git push origin main --tags
```

### 3. The `release.yml` workflow fires

On push of the `v0.1.7` tag, GitHub Actions automatically runs:

1. **Build** Windows release (Rust + frontend, ~5-10 min).
2. **Sign** the artifacts (`.exe` + `.msi`) using the private key from secrets.
3. **Create the GitHub Release** `RL Stats Overlay v0.1.7`.
4. **Upload** the `.exe`, the `.msi`, the `.exe.sig`, and a `latest.json` file containing the version + URL + signature.
5. The `latest.json` is reachable at `https://github.com/kevindjf/rl-stats-overlay/releases/latest/download/latest.json` — that's what user apps query on startup.

> ⏱️ **Total time**: ~10 min between the `git push` and updates being available to users.

### 4. Verify

- **GitHub Releases page**: the `v0.1.7` release must list 4 files (`.exe`, `.msi`, `.exe.sig`, `latest.json`).
- **Manifest**: open `https://github.com/kevindjf/rl-stats-overlay/releases/latest/download/latest.json` in your browser — you should see clean JSON with `version: "0.1.7"` and a non-empty `signature: "..."`.
- **Existing app**: launch a previous version (e.g. 0.1.6) — within 3 seconds, the **"🔔 New version 0.1.7 available"** banner should appear. Click **Install** → the app should download, restart, and show the new version.

## Failure cases

### `release.yml` fails with "signing key invalid"

Check that:
- `TAURI_SIGNING_PRIVATE_KEY` contains the **whole file** (starts with `untrusted comment: minisign secret key:` and several base64 lines).
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` matches the password used during `signer generate` (empty if you didn't set one).

### The banner doesn't show in the app

Check the DevTools (right-click the Settings window → Inspect → Console):
- Error `Update check failed: signature verification failed` → the pubkey in `tauri.conf.json` doesn't match the private key used by CI. Re-verify that both files (`.key` and `.key.pub`) come from the **same `signer generate`**.
- Error `Update check failed: 404` → `latest.json` wasn't published. Check the Releases page.
- No error, just nothing: your local version may already be up-to-date, or the version baked into `tauri.conf.json` of your build is >= the published tag.

### Users on 0.1.6 don't get the banner

Expected: versions **prior to the introduction of the updater** don't have code that queries `latest.json`. 0.1.7 is the **first version that activates the chain** — 0.1.6 users have to manually download 0.1.7 from GitHub Releases. From 0.1.7 onwards, all future updates (0.1.8, 0.2.0, etc.) are auto-notified.

To push 0.1.6 users to 0.1.7: announce in the README, on Discord / Twitter, and in the release notes.

## Versioning convention

[SemVer](https://semver.org/):
- `MAJOR.MINOR.PATCH`
- `0.X.Y` while the app is pre-1.0 (breaking changes between `MINOR` are acceptable).
- Tag must be prefixed with `v`: `v0.1.7`, not `0.1.7` (the workflow filters on `v*`).

## Pre-releases (rc, beta)

Tag with a suffix:

```bash
git tag v0.2.0-rc.1
git push origin v0.2.0-rc.1
```

The workflow detects the suffix and publishes the release **as `prerelease: true`** — invisible in `latest.json` (stable users aren't notified). To test on your account, manually download the prerelease `.exe`.
