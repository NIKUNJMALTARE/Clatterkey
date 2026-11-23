# Roadmap

Status values: `todo`, `spec`, `planned`, `building`, `built`, `done`.

| # | Feature | Outcome | Needs | Size | UI | Status |
|---|---|---|---|---|---|---|
| 01 | Project skeleton | The stack is installed, one test passes, `scripts/check.sh` runs format, lint, type check and tests, and the GitHub check runs it on every pull request | — | S | no | todo |
| 02 | Global key listener | The core reports every key press and release system-wide, ignoring auto-repeat, and a dev command prints them | 01 | M | no | todo |
| 03 | Click on keypress | Pressing any key in any app plays one built-in click with under 30 ms latency and overlapping presses do not cut each other off | 02 | M | no | todo |
| 04 | Sound pack format | The core loads a pack folder (`pack.json` plus audio) and plays different samples for letters, space, enter, backspace and modifiers, with optional key-up samples | 03 | M | no | todo |
| 05 | Bundled packs | The app ships with at least four packs (for example blue, brown, red, typewriter) with clear licences, selectable from a settings file | 04 | M | no | todo |
| 06 | Settings window | A settings window lets the user choose a pack, set volume, and turn sound on or off, and the choices are saved | 05 | M | yes | todo |
| 07 | Tray and background | The app lives in the tray with menu items to toggle sound, open settings and quit, and closing the window keeps it running | 06 | S | yes | todo |
| 08 | macOS permission onboarding | On macOS a first-run screen explains the permission, opens System Settings and detects when it is granted | 07 | M | yes | todo |
| 09 | Sound tuning | The user can set random pitch variation and key-up sounds on or off, and preview a pack from the settings window | 06 | S | yes | todo |
| 10 | Import your own pack | The user picks a folder of audio files or a pack folder and it appears in the pack list, with a clear message if it is invalid | 06 | M | yes | todo |
| 11 | Launch at login | A setting makes the app start hidden in the tray when the user logs in | 07 | S | yes | todo |
| 12 | Installers and release | A GitHub workflow builds a macOS `.dmg` and a Windows installer and attaches them to a release, and the README explains permissions and unsigned-app warnings | 08, 11 | M | no | todo |

## Now

Nothing started. Next: the user types `/feature 1`.
