# Mechanical Key Sounds

A small desktop app for macOS and Windows that plays the sound of a mechanical keyboard every time you press a key, in any app. It is for people who like the feel of a clicky keyboard on a quiet laptop or membrane keyboard, and for streamers and call-takers who want audible typing feedback. Users pick a sound pack (blue, brown, red switches, typewriter and so on), tune volume and pitch, and can import their own packs. Everything is local; there are no accounts.

@docs/roadmap.md

## Stack and commands

- Shell: Tauri 2.12 (Rust 1.96, tray icon, bundling)
- Core (Rust): `rdev` global key listener, `rodio` 0.23 audio playback, `serde_json` settings file
- UI: React 19.3 + TypeScript 7 + Vite 8, settings window only
- Tests: Vitest 5 + Testing Library (UI), `cargo test` (core)
- Format, lint, types: Biome 2.6 (UI), `cargo fmt` + `cargo clippy` (core), `tsc --noEmit`

| Task | Command |
|---|---|
| Install | `npm install` |
| Run locally | `npm run tauri dev` |
| Tests | `npm test && cargo test --manifest-path src-tauri/Cargo.toml` |
| All checks | `scripts/check.sh` (format, lint, type check, tests) |

Reasons and detail are in `docs/spec.md`. The Context7 IDs for every library are in its stack table; use them for any library question.

## Pipeline

One feature is one spec on one branch. Each stage is started by the user.

| The user types | Stage |
|---|---|
| `/feature N` | Feature spec, on a new branch `feat/NN-name` |
| `/feature N plan` | Implementation plan |
| `/feature N build` | Tests, build, review |
| `/ship` | Commit, push, pull request, merge |

- Do not start a stage on your own, and do not skip ahead.
- Source edits are allowed only on `feat/NN-name` after the user has typed `/feature N build`, or on a `fix/` branch for a bug fix the user asked for. A hook enforces this.
- Never create a `.approved` file, and never write source files through the shell to get round the hook.
- A bug fix changes no intended behaviour. Anything that adds or changes behaviour is a feature: add it to the roadmap first.

## Sources of truth

- **Architecture and stack:** `docs/spec.md`. If a feature has to change it, the feature spec says so and `docs/spec.md` is edited on the same branch.
- **Behaviour of a feature:** `docs/features/NN-name/spec.md`.
- **Order and status:** `docs/roadmap.md`.

## Working rules

- Ask before: adding a dependency that is not in the spec or the feature plan, changing the stack, deleting data, anything destructive.
- Before launching any agent that reviews code or writes or runs tests, or any agent whose model is being chosen, ask the user which model with AskUserQuestion: Fable, Opus, Sonnet or Haiku, recommended first, one question per agent in a single dialog. Pass the answer as that launch's `model`. Ask again each time. Agents that only inherit the session's model and do no review or testing need no question.
- Never commit, push or merge outside `/ship`.
- Commits, pull requests and branch names never mention Claude or AI, and carry no co-author line.
- Verify before calling something done, and say what was not verified.
