# Mechanical Key Sounds

A small desktop app for macOS and Windows that plays the sound of a mechanical keyboard every time you press a key, in any app. Everything is local; there are no accounts.

The app is under construction. So far, while it is running, every key you press in any app plays a sound: a built-in click, or a sound pack you point it at during development (see below). Closing the window quits it.

On macOS the app can only hear key presses once it has the Accessibility / Input Monitoring permission in System Settings, Privacy & Security. Without it the app runs and is silent. It notices only that a key went down: it never records, stores or sends which key it was.

## Development

You need Node 22 or later and Rust 1.96 (installed by `rustup` from `rust-toolchain.toml`). On Linux you also need the [Tauri system packages](https://tauri.app/start/prerequisites/) and `libasound2-dev`.

| Task | Command |
|---|---|
| Install | `npm install` |
| Run locally | `npm run tauri dev` |
| Print key events (dev tool) | `npm run keys` |
| Tests | `npm test && cargo test --manifest-path src-tauri/Cargo.toml` |
| All checks | `scripts/check.sh` |

`npm install` also points git at `.githooks/`, so the checks run before every push. The hook runs them in check-only mode, so a push with badly formatted code is refused; run `scripts/check.sh` to fix the formatting, commit, and push again.

`npm run keys` prints one line for every key press and release in any app (`down KeyA`, `up KeyA`) until you stop it with Ctrl+C. It is a development tool and is not part of the app. On macOS the terminal you run it from needs the Accessibility / Input Monitoring permission in System Settings, Privacy & Security; without it nothing is printed. It prints which physical key was pressed, never the character typed, and stores nothing.

When you start the app with `npm run tauri dev` on macOS, it is the terminal you run it from that needs the permission above, as for `npm run keys`.

### Trying a sound pack

Until packs can be chosen in the app, set `MKS_PACK` to a pack folder when you start it:

```sh
MKS_PACK=/path/to/my-pack npm run tauri dev
```

A pack is a folder with a `pack.json` and audio files (WAV, OGG Vorbis, MP3 or FLAC):

```json
{
  "name": "My pack",
  "license": "CC0-1.0",
  "keys": { "default": ["a.wav", "b.wav"], "space": ["space.wav"] },
  "up": { "default": ["up.wav"] }
}
```

Groups are `default`, `space`, `enter`, `backspace` and `modifier`; any group left out plays `default`, and `up` is optional. If the pack is invalid the built-in click plays and one line starting `sound pack:` says what is wrong. The full format is in `docs/spec.md`. Test packs live in `src-tauri/tests/fixtures/packs/`.

`scripts/check.sh` formats, lints, type-checks and tests the UI and the core. It rewrites badly formatted files when run locally and only reports them when the `CI` environment variable is set.
