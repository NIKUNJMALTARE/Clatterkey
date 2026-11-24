# ClickClak

A small desktop app for macOS and Windows that plays the sound of a mechanical keyboard every time you press a key, in any app. Everything is local; there are no accounts.

The app notices only that a key went down or came up. It never records, stores or sends which key it was, and it has no network code.

## Install

Download the installer for your system from the [Releases page](https://github.com/NIKUNJMALTARE/Mechanical_Keyboard/releases).

| System | Needs | File |
|---|---|---|
| macOS | macOS 12 or later on a Mac with Apple silicon | `ClickClak_<version>_aarch64.dmg` |
| Windows | Windows 10 or later, 64-bit (x64) | `ClickClak_<version>_x64-setup.exe` |

A Mac with an Intel processor is not supported.

### macOS

1. Open the `.dmg` and drag **ClickClak** onto the **Applications** shortcut beside it.
2. Open the app from Applications. The first time, macOS refuses, because the app is not from an identified developer:
   - **macOS 15 or later:** close the message, open System Settings, Privacy & Security, scroll down to the line about ClickClak and press **Open Anyway**. Confirm with your password or Touch ID.
   - **macOS 12 to 14:** hold Control and click the app in Applications, choose **Open**, then **Open** again in the message.

   This is needed once. After that the app opens like any other.
3. The app then asks for the Input Monitoring permission, which it needs to hear key presses. See [The macOS permission](#the-macos-permission) below.

### Windows

1. Run the setup `.exe`. Microsoft Defender SmartScreen shows "Windows protected your PC": press **More info**, then **Run anyway**.
2. Follow the installer. It installs for your user account only and needs no administrator rights. If the WebView2 runtime is missing (it is part of Windows 11 and of an up-to-date Windows 10), the installer downloads it.
3. The app starts from the installer's last page or from the Start menu. Typing sounds at once; there is no permission to give.

### Why the warnings

The installers are not signed with a paid developer certificate, so macOS and Windows cannot name who made them and warn before the first start. The source is in this repository, and every release carries a `SHA256SUMS.txt` with the SHA-256 sum of each installer. To compare, run this on the file you downloaded and check the result against the line for that file (upper and lower case do not matter):

- macOS, in Terminal: `shasum -a 256 ~/Downloads/ClickClak_<version>_aarch64.dmg`
- Windows, in PowerShell: `Get-FileHash "$HOME\Downloads\ClickClak_<version>_x64-setup.exe"`

## Updating

The app does not look for new versions. To update, download the newer installer and install it over the old one; your settings and imported packs are kept.

- **macOS:** quit the app from the tray, then drag the new app onto Applications and choose Replace. macOS treats each version as a different app, so the first-open step above and the Input Monitoring permission are needed again: in System Settings, Privacy & Security, Input Monitoring, select the old ClickClak entry, remove it with the minus button, then open the new app and grant the permission as on first run.
- **Windows:** run the new setup `.exe`. If the app is running, the installer asks to close it first.

## Uninstalling

- **Windows:** Settings, Apps, Installed apps, ClickClak, Uninstall. This also removes the Launch at login entry. Your settings and imported packs are kept unless you tick the uninstaller's box to delete the application data; they are in `%APPDATA%\com.clickclak.app\`.
- **macOS:** turn **Launch at login** off if it was on, quit the app from the tray, and drag it from Applications to the Bin. Two things are left behind and do nothing; delete them if you want them gone:
  - your settings and imported packs, in `~/Library/Application Support/com.clickclak.app/`;
  - if Launch at login was still on, the file `~/Library/LaunchAgents/ClickClak.plist`.

## Coming from Mechanical Key Sounds

ClickClak is the same app under a new name. To your computer it is a new app, not an update, so it is installed beside the old one.

1. Quit Mechanical Key Sounds from the tray. If both run at once, every key sounds twice.
2. Install ClickClak as described under Install, and open it. The first time it starts it finds your old settings and imported packs and copies them, so it looks and sounds as before. If Launch at login was on, it stays on, now for ClickClak.
3. **macOS:** the Input Monitoring permission has to be given again, to ClickClak; the app asks as on a first run. In the Input Monitoring list you can select the old Mechanical Key Sounds entry and remove it with the minus button.
4. Remove the old app:
   - **macOS:** drag `Mechanical Key Sounds.app` from Applications to the Bin.
   - **Windows:** Settings, Apps, Installed apps, Mechanical Key Sounds, Uninstall. Do this after ClickClak has been started once, or leave the uninstaller's box to delete the application data unticked, so that your settings are still there to be copied.

Nothing of the old app is changed or removed by ClickClak, except its Launch at login entry. Once ClickClak shows your settings, the old folder can be deleted:

- macOS: `~/Library/Application Support/com.mechanicalkeysounds.app/`
- Windows: `%APPDATA%\com.mechanicalkeysounds.app\`

The copy is made only once, at the first start, and only if ClickClak has no settings of its own yet. If it did not work and ClickClak started with the default settings, copy them by hand: quit ClickClak, then copy `settings.json` and the `packs` folder from the old folder into the new one, replacing what is there, and start ClickClak again.

| | From | To |
|---|---|---|
| macOS | `~/Library/Application Support/com.mechanicalkeysounds.app/` | `~/Library/Application Support/com.clickclak.app/` |
| Windows | `%APPDATA%\com.mechanicalkeysounds.app\` | `%APPDATA%\com.clickclak.app\` |

## Using the app

While the app is running, every key you press in any app plays a sound from one of four bundled sound packs (blue, brown and red switches, and a typewriter) or from a pack you imported. The settings window chooses the pack, sets the volume and turns the sound on or off; every change is heard on the next key press and is saved.

Three more controls tune the sound:

- **Pitch variation** (Off to 30%) plays each sound a little faster or slower, chosen at random within that range, so repeated keys do not sound identical. Faster is higher and slightly shorter.
- **Key-up sounds** turns the sound of a key being released on or off. Typewriter has no such sounds, and the window says so.
- **Preview** plays a short typed phrase with the pack and settings in force, so you can hear a pack without typing. It plays even when Sound is off, and on macOS before the permission below is given.

### Importing a pack

**Import pack…** in the settings window opens the system's folder picker. The folder you choose is checked, copied into the app's own data, added to the list and selected. Your folder is only read, and later edits to it have no effect until you import it again.

Two kinds of folder work:

- **A pack folder**: a `pack.json` and the audio files it names (the format is in `docs/spec.md`). Only those files are copied.
- **A plain folder of audio files**: its own `.wav`, `.ogg`, `.mp3` and `.flac` files, not its subfolders. Each file is assigned by its name, ignoring case:
  - a name ending in `-up`, `_up` or ` up` is a key-up sound (`click-up.wav`);
  - a name starting with `space`, `enter`, `backspace` or `modifier` is for those keys (`space.wav`, `Space 2.wav`, `enter-up.wav`);
  - every other file is an ordinary key sound, and there must be at least one.

  A file name may not contain `:` or `\` (Finder shows `:` as `/`); rename such a file first.

  The pack takes the folder's name and is listed as "Licence not stated".

The limits are those of every pack: each file at most 10 MB, 3 seconds and 96 kHz; at most 100 files and 60 seconds of audio. One bad file refuses the whole folder, and a notice says which file and why; the pack that was playing keeps playing.

Importing a folder with the same name again replaces the earlier copy. **Remove** on an imported pack's row asks once, then deletes the app's copy; your own folder is not touched. If that pack was playing, Brown switches takes over. The bundled packs cannot be removed.

The copies are kept in `packs/` beside `settings.json` (see Settings below), one folder per pack, named by an id made from your folder's name (`My Clicks` becomes `my-clicks`).

### The macOS permission

On macOS the app can only hear key presses once it has the Input Monitoring permission (System Settings, Privacy & Security, Input Monitoring). It asks for nothing else, and it notices only that a key went down: it never records, stores or sends which key it was.

While the permission is missing, the window shows a screen that explains this in place of the settings. **Open System Settings** takes you to the Input Monitoring list; turn ClickClak on there and the sound starts by itself within a second or so. If it does not, the screen offers **Restart now**. **Set up later** shows the settings for now, with a reminder at the top; there is no sound until the permission is given.

If ClickClak is already switched on in the list and the app is still silent, the entry is left over from an older copy of the app. Select it in the list, remove it with the minus button, and add the app again.

### The tray

The app lives in the tray: the menu bar on macOS, the notification area on Windows. Closing the settings window does not quit it; keys keep sounding.

- **Sound** turns the sound on or off. The icon is dimmed while it is off.
- **Settings…** shows the settings window. On Windows a left click on the icon does the same.
- **Quit ClickClak** stops the app.

On macOS the Dock icon is there only while the settings window is open. Starting the app while it is already running shows its settings window; it does not start a second copy.

### Launch at login

Turn on **Launch at login** at the bottom of the settings window and the app starts by itself when you log in. Such a start is hidden: the icon appears in the tray, keys sound, and no window opens. Starting the app yourself always opens the window.

- macOS lists the app under System Settings, General, Login Items, and may show a "Background Items Added" notification when you turn the switch on.
- Windows lists it under Task Manager, Startup apps.
- On macOS, if the Input Monitoring permission is missing at login, the window opens after all to explain it.
- If you disable the entry in Windows Task Manager, the app's switch is off the next time the app starts. macOS does not tell the app when you switch the item off in Login Items: the app is then not started at login although its switch still shows on. Switch it back on in Login Items.
- A development build (`npm run tauri dev`) never registers itself; the switch is only saved there.

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

`npm run keys` prints one line for every key press and release in any app (`down KeyA`, `up KeyA`) until you stop it with Ctrl+C. It is a development tool and is not part of the app. On macOS the terminal you run it from needs the Input Monitoring permission in System Settings, Privacy & Security; without it nothing is printed. It prints which physical key was pressed, never the character typed, and stores nothing.

When you start the app with `npm run tauri dev` on macOS, it is the terminal you run it from that needs the permission above, as for `npm run keys`, and the permission screen shows until that terminal has it. To see the first-run screen again with a built app, take its permission away with `tccutil reset ListenEvent com.clickclak.app`.

### Settings

The settings window has a Sound switch, a volume slider, a pitch variation slider, a Key-up sounds switch, Import pack… and Preview buttons and the list of sound packs. There is no Save button: a change applies at once and is written to `settings.json`, which is created with the defaults on first run:

- macOS: `~/Library/Application Support/com.clickclak.app/settings.json`
- Windows: `%APPDATA%\com.clickclak.app\settings.json`

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

`pack_id` is one of `blue`, `brown`, `red` or `typewriter`, or the id of an imported pack. `pitch_variation` is from 0.0 (off) to 0.3. `launch_at_login` is compared with the system's own list when the app starts: a registration that is gone or disabled there turns it off, and with the setting off a registration that is still there is removed. The file is read only when the app starts, so an edit made by hand while the app runs has no effect and is overwritten by the next change in the window.

An unknown pack plays `brown`, and a broken settings file is left as it is with the defaults in use. Either way the window shows a notice, and one line starting `sound pack:` or `settings:` is printed. A broken file is replaced the first time a setting is changed in the window.

To look at the window in a browser without the app, run `npm run dev` and open `http://localhost:5173`. There is no core there, so an in-memory stand-in answers (development builds only); add `?state=` with one of `loading`, `load-error`, `no-audio`, `audio-error`, `settings-invalid`, `fields-reset`, `pack-fallback`, `pack-load-error`, `save-error`, `no-packs`, `permission-needed`, `permission-restart`, `permission-open-fails`, `no-key-up`, `preview-error`, `imported`, `import-error`, `remove-error`, `launch-at-login` or `autostart-error` to see that state. There, Import pack… adds a made-up pack with no picker. To play the part of the tray there, run `__mksFake.outsideChange({ enabled: false })` in the browser console; to play the part of System Settings, run `__mksFake.setPermission("granted")` (or `"needed"`, `"restart_needed"`).

### The bundled packs

The packs in `src-tauri/resources/packs/` are synthesised from code by the generator in `src-tauri/examples/make_packs/`, with no recordings, and are released under CC0 (see the `LICENSE.md` beside them). The generator is deterministic and its output is committed, so building the app never runs it. After changing it, run `npm run make-packs` and commit the regenerated files; a test checks that they match the generator.

A pack is a folder with a `pack.json` and audio files (WAV, OGG Vorbis, MP3 or FLAC); the full format is in `docs/spec.md`. Test packs live in `src-tauri/tests/fixtures/packs/`.

`scripts/check.sh` checks that the version is the same in every file that holds it, then formats, lints, type-checks and tests the UI and the core. It rewrites badly formatted files when run locally and only reports them when the `CI` environment variable is set.

### Releasing

A release is a version tag. Pushing the tag builds both installers in GitHub Actions (`.github/workflows/release.yml`) and attaches them to a draft release.

1. Set the new version in `package.json`, `package-lock.json` (`npm version <X.Y.Z> --no-git-tag-version` does both) and `src-tauri/Cargo.toml`. `scripts/check.sh` fails while they differ. Run `scripts/check.sh` once afterwards and commit `src-tauri/Cargo.lock` with the rest, since cargo writes the new version into it. Merge that through a pull request as usual.
2. On `main`, tag that commit and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The workflow first checks that the tag is `v` plus the project's version, and builds nothing if it is not.
4. It builds the macOS `.dmg` (Apple silicon, ad-hoc signed) and the Windows setup `.exe` (x64), and checks each.
5. Only if both builds pass, it creates a draft release, "ClickClak X.Y.Z", with the two installers and `SHA256SUMS.txt`. A version with a pre-release part (`v0.2.0-beta.1`) is marked as a pre-release.
6. Open the draft on the Releases page, write the notes, and press **Publish release**. Nothing is public before that.

Running the tag's workflow run again (Re-run all jobs on that run in the Actions tab) replaces the files of a draft and leaves its notes alone. A published release is never changed: release a new version instead.

To try the build without releasing, start the **release** workflow by hand from the Actions tab (Run workflow, on any branch or tag). A run started by hand never creates or changes a release, even on a tag. It builds both installers and keeps them as artifacts of that run for 7 days, and creates no release. A pull request that changes the workflow, `src-tauri/tauri.conf.json`, `src-tauri/windows/` or `scripts/check-version.mjs` does the same by itself. On your own machine, `npm run tauri build` makes the installer for the system you are on, in `src-tauri/target/release/bundle/`.
