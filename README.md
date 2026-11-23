# Mechanical Key Sounds

A small desktop app for macOS and Windows that plays the sound of a mechanical keyboard every time you press a key, in any app. Everything is local; there are no accounts.

The app is under construction. So far, while it is running, every key you press in any app plays a sound from one of four bundled sound packs: blue, brown and red switches, and a typewriter. The settings window chooses the pack, sets the volume and turns the sound on or off; every change is heard on the next key press and is saved.

On macOS the app can only hear key presses once it has the Accessibility / Input Monitoring permission in System Settings, Privacy & Security. Without it the app runs and is silent. It notices only that a key went down: it never records, stores or sends which key it was.

### The tray

The app lives in the tray: the menu bar on macOS, the notification area on Windows. Closing the settings window does not quit it; keys keep sounding.

- **Sound** turns the sound on or off. The icon is dimmed while it is off.
- **Settings…** shows the settings window. On Windows a left click on the icon does the same.
- **Quit Mechanical Key Sounds** stops the app.

On macOS the Dock icon is there only while the settings window is open. Starting the app while it is already running shows its settings window; it does not start a second copy.

## Development

You need Node 22 or later and Rust 1.96 (installed by `rustup` from `rust-toolchain.toml`). On Linux you also need the [Tauri system packages](https://tauri.app/start/prerequisites/) and `libasound2-dev`.

| Task | Command |
|---|---|
| Install | `npm install` |
| Run locally | `npm run tauri dev` |
| Print key events (dev tool) | `npm run keys` |
| Regenerate the bundled packs | `npm run make-packs` |
| Regenerate the tray icons | `npm run make-tray-icons` |
| Tests | `npm test && cargo test --manifest-path src-tauri/Cargo.toml` |
| All checks | `scripts/check.sh` |

`npm install` also points git at `.githooks/`, so the checks run before every push. The hook runs them in check-only mode, so a push with badly formatted code is refused; run `scripts/check.sh` to fix the formatting, commit, and push again.

`npm run keys` prints one line for every key press and release in any app (`down KeyA`, `up KeyA`) until you stop it with Ctrl+C. It is a development tool and is not part of the app. On macOS the terminal you run it from needs the Accessibility / Input Monitoring permission in System Settings, Privacy & Security; without it nothing is printed. It prints which physical key was pressed, never the character typed, and stores nothing.

When you start the app with `npm run tauri dev` on macOS, it is the terminal you run it from that needs the permission above, as for `npm run keys`.

### Settings

The settings window has a Sound switch, a volume slider and the list of sound packs. There is no Save button: a change applies at once and is written to `settings.json`, which is created with the defaults on first run:

- macOS: `~/Library/Application Support/com.mechanicalkeysounds.app/settings.json`
- Windows: `%APPDATA%\com.mechanicalkeysounds.app\settings.json`

```json
{
  "enabled": true,
  "pack_id": "brown",
  "volume": 0.8,
  "pitch_variation": 0.0,
  "key_up_sounds": true,
  "launch_at_login": false
}
```

`pack_id` is one of `blue`, `brown`, `red` or `typewriter`. `enabled`, `pack_id` and `volume` have an effect so far; the others are kept for later features. The file is read only when the app starts, so an edit made by hand while the app runs has no effect and is overwritten by the next change in the window.

An unknown pack plays `brown`, and a broken settings file is left as it is with the defaults in use. Either way the window shows a notice, and one line starting `sound pack:` or `settings:` is printed. A broken file is replaced the first time a setting is changed in the window.

To look at the window in a browser without the app, run `npm run dev` and open `http://localhost:5173`. There is no core there, so an in-memory stand-in answers (development builds only); add `?state=` with one of `loading`, `load-error`, `no-audio`, `audio-error`, `settings-invalid`, `fields-reset`, `pack-fallback`, `pack-load-error`, `save-error` or `no-packs` to see that state. To play the part of the tray there, run `__mksFake.outsideChange({ enabled: false })` in the browser console.

### The bundled packs

The packs in `src-tauri/resources/packs/` are synthesised from code by the generator in `src-tauri/examples/make_packs/`, with no recordings, and are released under CC0 (see the `LICENSE.md` beside them). The generator is deterministic and its output is committed, so building the app never runs it. After changing it, run `npm run make-packs` and commit the regenerated files; a test checks that they match the generator.

A pack is a folder with a `pack.json` and audio files (WAV, OGG Vorbis, MP3 or FLAC); the full format is in `docs/spec.md`. Test packs live in `src-tauri/tests/fixtures/packs/`.

`scripts/check.sh` formats, lints, type-checks and tests the UI and the core. It rewrites badly formatted files when run locally and only reports them when the `CI` environment variable is set.
