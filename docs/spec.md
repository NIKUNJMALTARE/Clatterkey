# Mechanical Key Sounds — overall spec

## 1. Summary

A background desktop app for macOS and Windows. It listens for key presses system-wide and immediately plays a mechanical-keyboard sound from the selected sound pack, whatever app has focus. Packs are customisable (volume, pitch variation, key-up sounds) and users can import their own. It is finished when a user can install it, grant the OS permission once, pick a pack, and hear low-latency clicks in every app, with the app living in the tray and starting at login.

## 2. Users and main flows

Users: people who type a lot and want the sound of a mechanical keyboard (typists, streamers, remote workers). No sign-in.

1. **First run (macOS):** open app, see a permission screen explaining Input Monitoring / Accessibility, click "Open System Settings", grant it, the app detects the grant and starts clicking.
2. **First run (Windows):** open app, clicks work at once, the settings window shows the default pack.
3. **Change pack:** open settings from the tray, pick a pack from the list, press the preview button or just type, hear the new sound.
4. **Tune the sound:** move volume, pitch-variation and key-up-sound controls; changes apply to the next key press and are saved.
5. **Import a pack:** click "Import pack", choose a folder with a `pack.json` and audio files (or a folder of audio files, which the app wraps in a pack), see it appear in the list.
6. **Pause and quit:** tray menu toggles sound on or off, opens settings, or quits. Closing the settings window leaves the app running in the tray.

## 3. Scope

**In:** global key press and release detection; bundled sound packs; pack format and loader; volume, random pitch variation, key-up sounds; settings window; tray icon with on/off toggle; macOS permission onboarding; import of user packs; launch at login; installers for macOS and Windows.

**Out:** Linux; accounts, cloud sync, a pack marketplace; recording the user's own keyboard; per-app rules; mouse click sounds; a browser or web version (a web page cannot hear keys pressed in other apps); mobile.

## 4. Tech stack

Final. One choice per layer.

| Layer | Choice | Version | Why | Context7 ID |
|---|---|---|---|---|
| Language (core) | Rust | 1.96 | Needed for low-latency audio and OS key hooks | — |
| Language (UI) | TypeScript | 7.0 | Typed UI code | `/microsoft/typescript` |
| App shell | Tauri | 2.12 (`tauri` crate 2.12, `@tauri-apps/cli` 2.12) | Small native app, tray, installers for both OSes, Rust core beside a web UI | `/tauri-apps/tauri-docs` |
| UI framework | React | 19.3 | Widely used, fits Tauri templates | `/react/react` |
| UI build | Vite | 8.3 | Standard Tauri front-end tool | `/vitejs/vite` |
| Global key capture | rdev (Narsil), git `main` | 0.5.x | Global listener on macOS and Windows; see Risks | `/narsil/rdev` |
| Audio playback | rodio | 0.23 | Mixer plays overlapping sounds from decoded in-memory buffers | `/rustaudio/rodio` |
| Data | JSON settings file in the app config dir, written with `serde_json` 1.0 | — | A few settings, no database needed | — |
| Auth | None | — | Local only | — |
| Plugins | `tauri-plugin-autostart` 2.7, `tauri-plugin-dialog` 2.8 | as stated | Launch at login; folder picker for import | `/tauri-apps/plugins-workspace` |
| Tests | Vitest 5.1 + `@testing-library/react` 16 + jsdom; `cargo test` | as stated | Fast UI tests; unit and integration tests for the core | `/vitest-dev/vitest` |
| Format, lint, types | Biome 2.6 (format + lint), `tsc --noEmit`, `cargo fmt`, `cargo clippy -- -D warnings` | as stated | One fast tool for TS, standard tools for Rust | `/biomejs/biome` |
| Hosting | None. Installers published as GitHub Releases | — | Desktop app | — |

Package manager: npm (pnpm is not installed on this machine).

## 5. Architecture

Two parts in one process:

- **Core (`src-tauri/`, Rust):**
  - `keys`: runs the `rdev` listener on its own thread and turns events into `KeyEvent { code, kind: Down|Up }`. Ignores key auto-repeat (a held key sounds once).
  - `audio`: owns the output stream and mixer. Sound packs are decoded once into memory when selected; a key event picks a sample and adds it to the mixer. No disk reads or decoding on the key path.
  - `packs`: reads a pack folder (`pack.json` plus audio files) and maps key groups to samples. Bundled packs live in `src-tauri/resources/packs/`; imported ones in `<app data>/packs/`.
  - `settings`: load and save the settings file (selected pack, volume, pitch variation, key-up on/off, enabled, launch at login).
  - `commands`: Tauri commands the UI calls (`list_packs`, `get_settings`, `set_settings`, `import_pack`, `preview`, `permission_status`).
  - `tray`: tray icon and menu.
- **UI (`src/`, React):** one settings window. It only shows and edits settings; it never touches audio or keys. It talks to the core through typed wrappers in `src/api/`.

Flow: key press → OS → `rdev` thread → `audio` picks sample → mixer → speakers. The UI is not on this path, so closing the window does not affect latency.

Boundaries: only `keys` talks to `rdev`; only `audio` talks to `rodio`; the UI talks to the core only through `commands`. The key path must never block on locks held by the UI side (use a lock-free or short-lived snapshot of the settings).

Pack format (`pack.json`): `{ "name", "author", "license", "keys": { "default": [files], "space": [files], "enter": [files], "backspace": [files], "modifier": [files] }, "up": { same shape, optional } }`. Several files per group are chosen at random.

Folder layout:

```
src/                 React UI, src/api/ typed core wrappers
src-tauri/src/       Rust modules above
src-tauri/resources/packs/   bundled packs
docs/                spec, roadmap, feature specs
scripts/check.sh     all checks
```

## 6. Data model and external services

Settings (one JSON file, `settings.json`): `enabled: bool`, `pack_id: string`, `volume: 0.0–1.0`, `pitch_variation: 0.0–0.3`, `key_up_sounds: bool`, `launch_at_login: bool`.

Pack (on disk): see the format above; `id` is the folder name.

No external services and no network access. The app must work offline. If no audio output device exists, the core keeps running and retries when the device list changes; it shows a notice in the settings window.

## 7. Testing and checks

- **Rust unit tests** (`#[cfg(test)]` beside the code): pack parsing and validation, key-to-group mapping, auto-repeat filtering, settings load and save, sample choice. Audio output is behind a trait so tests use a fake sink.
- **UI tests** (Vitest + Testing Library, `*.test.tsx` beside the component): settings controls call the right commands; the pack list renders; the permission screen shows on the denied state. The core is mocked at `src/api/`.
- **Manual checks:** real key capture, latency and OS permission flows cannot run in CI. Each feature that touches them lists a manual checklist in its spec.
- `scripts/check.sh` runs, in order: `biome check`, `tsc --noEmit`, `npm test`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`. The GitHub check runs the same script on every pull request, on `ubuntu-latest` with the Tauri Linux build dependencies installed (the code must compile there even though Linux is not a supported target; key capture is compiled out behind `cfg`).

## 8. Design

No design was given. Direction for version one: a compact, utilitarian settings window, about 420×560, light and dark themes following the OS, one accent colour, system font stack. Tokens are defined as CSS variables in `src/styles/tokens.css` by feature 06 and then become part of this section. No marketing site is in scope, so there is no design-direction feature.

## 9. Deployment and environments

Installers: a `.dmg` for macOS (Apple silicon and Intel) and an `.msi`/`.exe` for Windows, built with `tauri build` in GitHub Actions and attached to a GitHub Release (feature 12). Environment variables: none at run time. Build-time signing secrets (names only): `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`; added only if the user chooses to sign.

## 10. Risks

- **`rdev` upkeep and macOS threading:** the crates.io release is old and `listen` on macOS has caveats (needs Accessibility/Input Monitoring; events are silently dropped without it, with no error). Mitigation: use the maintained git `main`, wrap it in the `keys` module so it can be swapped, check permission explicitly in feature 08, and test on real hardware.
- **Latency:** sound that lags the key feels broken. Mitigation: pre-decoded samples, a small output buffer, measure in feature 03 (target under 30 ms from key event to audio start).
- **Pack licensing:** bundled audio must be redistributable. Mitigation: only packs with a clear open licence or ones the user recorded; the licence is stored in `pack.json` and shown in the UI.
- **Unsigned installers:** macOS Gatekeeper and Windows SmartScreen warn on unsigned apps. Mitigation: document it in feature 12 and sign if the user has certificates.
- **Privacy perception:** a global key listener looks like a keylogger. Mitigation: the core records only the key kind (never stores or transmits key values), has no network code, and the README says so.

## 11. Assumptions and open questions

Assumptions (decided without asking):

- Project name `mechanical-key-sounds`; the product name can change later.
- npm as the package manager; React with Vite; Biome rather than ESLint and Prettier.
- Sounds play for key down; key-up sounds are optional and only for packs that have them.
- Auto-repeat of a held key does not retrigger the sound.
- Imported packs are copied into the app data folder, not referenced in place.
- macOS 12 or later and Windows 10 or later.
- No telemetry, no auto-update in version one.

Open questions:

- **Sound source:** where do the bundled packs come from? Options: record your own, or use a pack with a confirmed open licence. The skeleton and pack loader can be built with a placeholder click; feature 05 needs the answer.
- Apple Developer and Windows signing certificates: do you have them, or should installers ship unsigned?
