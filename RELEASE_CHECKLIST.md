# Eous Modify Release Checklist

## One-time setup (already done, recorded here for reference)

- [x] Signing keypair generated via `npx tauri signer generate`.
   - Private key: `~/.tauri/eous-modify-updater.key`
   - Password: `~/.tauri/eous-modify-updater.password.txt` (same folder) — a random 32-character
     string.
   - **Back both of these up somewhere durable (a password manager, an encrypted drive) that isn't
     just this one machine.** If you lose them, you can never sign a valid update again — you'd have
     to generate a new keypair, ship the new public key in one final manually-distributed release,
     and every install would need to pick that up before auto-update works again.
- [x] Public key + GitHub Releases endpoint set in `src-tauri/tauri.conf.json`
      (`plugins.updater.pubkey` / `plugins.updater.endpoints`).
- [x] **GitHub repo secrets** (Settings → Secrets and variables → Actions, on
      `khawpunkp/Eous-Modify`) —
      `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` added via the web UI.

## Every release, from here on

1. **Decide the new version number** (semver — bump patch for fixes, minor for features).
2. **Bump the version in all four places** — they must match exactly:
   - `package.json` (`version`)
   - `src-tauri/Cargo.toml` (`[package] version`)
   - `src-tauri/Cargo.lock` (the `eous-modify` package entry — `cargo test` or `cargo check` updates
     it for you once `Cargo.toml` is edited)
   - `src-tauri/tauri.conf.json` (`version`)
3. **Write the release notes in `CHANGELOG.md`**, as a `## <version>` section. This is not optional and
   it cannot be deferred — see the warning below. The build fails if the tag has no matching section.
4. Commit the bump and the notes (e.g. `git commit -m "chore: set version to 3.1.0"`).
5. Push the commit, then **tag it and push the tag** — the tag is what triggers the release build:
   ```
   git tag v3.1.0
   git push origin v3.1.0
   ```
6. Watch the run under the repo's **Actions** tab. It reads your `CHANGELOG.md` section, builds the
   Windows `.msi`, signs it with the private key (from the secrets above), and creates a **draft**
   release with the installer, its `.sig` signature file, and a generated `latest.json` attached.
7. Go to **Releases** on GitHub, open the new draft, confirm the `.msi` + `.sig` + `latest.json` are
   all attached and the notes look right, then **click "Publish release."**
   - The draft is intentional — nothing is served to existing installs until you publish it. The
     update endpoint (`.../releases/latest/download/latest.json`) only resolves to a _published_,
     non-prerelease release.
8. Existing installs will pick up the new version next time they check (on startup, or via the
   "Check for Updates" button in Settings).

> **Editing the release description on GitHub does not change what the update prompt shows.**
>
> The in-app prompt reads the `notes` field inside `latest.json`, and that file is generated during the
> build from whatever `CHANGELOG.md` said at the time. The release description and `latest.json` are two
> separate copies of the text; the web UI only edits the first. If you publish with the wrong notes, the
> only ways to fix it are to replace the `latest.json` asset on that release by hand, or to ship another
> version. So get `CHANGELOG.md` right in step 3, before tagging.

## If something goes wrong

- **Build/sign fails in Actions**: check the secrets are set correctly (step 0 above) — a missing or
  wrong `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is the most common cause.
- **Published release, but the app doesn't detect it**: confirm the release isn't marked
  "pre-release" and isn't still a draft — both are excluded from the `latest` endpoint.
- **Never publish a draft you haven't reviewed** — once published, it's live to every install that
  checks for updates.
