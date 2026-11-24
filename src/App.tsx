import { useEffect, useRef, useState } from "react";
import { KeyUpSwitch } from "./components/KeyUpSwitch";
import { LaunchSwitch } from "./components/LaunchSwitch";
import { type Notice, Notices } from "./components/Notices";
import { PackList } from "./components/PackList";
import {
	PERMISSION_HEADING_ID,
	PermissionScreen,
} from "./components/PermissionScreen";
import { PitchSlider } from "./components/PitchSlider";
import { SOUND_SWITCH_ID, SoundSwitch } from "./components/SoundSwitch";
import { VolumeSlider } from "./components/VolumeSlider";
import { type Ready, useSettings } from "./useSettings";

/** Whether macOS is keeping the app from hearing keys. */
function needsPermission(view: Ready): boolean {
	return view.permission === "needed" || view.permission === "restart_needed";
}

/** The notices to show: the permission first, then worst first. */
function noticesFor(
	view: Ready,
	dismissed: ReadonlySet<string>,
	dismiss: (id: string) => void,
	setUpPermission: () => void,
): Notice[] {
	const notices: Notice[] = [];
	const { startup } = view;

	// Shown only after "Set up later": otherwise the permission screen is.
	if (needsPermission(view)) {
		notices.push({
			id: "permission",
			tone: "warning",
			text: "Mechanical Key Sounds cannot hear your keys yet. It needs the Input Monitoring permission.",
			action: { label: "Set up", onAction: setUpPermission },
		});
	}

	if (view.audio === "no_device") {
		notices.push({
			id: "audio",
			tone: "warning",
			text: "No audio output device found. Sounds will play when one is connected.",
		});
	} else if (view.audio === "failed") {
		notices.push({
			id: "audio",
			tone: "warning",
			text: "The audio output could not be opened. Retrying…",
		});
	}
	if (view.packError) {
		notices.push({
			id: "pack-error",
			tone: "error",
			text: `Could not load ${view.packError.name}: ${view.packError.reason}`,
			onDismiss: view.dismissPackError,
		});
	}
	if (view.importError) {
		notices.push({
			id: "import-error",
			tone: "error",
			// The core's words, whole: `Could not import "<folder>": <reason>`.
			text: view.importError,
			onDismiss: view.dismissImportError,
		});
	}
	if (view.removeError) {
		notices.push({
			id: "remove-error",
			tone: "error",
			text: view.removeError,
			onDismiss: view.dismissRemoveError,
		});
	}
	if (view.saveError) {
		notices.push({
			id: "save-error",
			tone: "error",
			text: `Could not save your settings: ${view.saveError}. Changes will last until the app quits.`,
		});
	}
	if (view.changeError) {
		notices.push({
			id: "change-error",
			tone: "error",
			text: `Could not change the setting: ${view.changeError}`,
			onDismiss: view.dismissChangeError,
		});
	}
	if (view.previewError) {
		notices.push({
			id: "preview-error",
			tone: "error",
			text: `Could not play the preview: ${view.previewError}`,
			onDismiss: view.dismissPreviewError,
		});
	}
	if (!dismissed.has("settings-file")) {
		if (startup.settings_unreadable) {
			notices.push({
				id: "settings-file",
				tone: "warning",
				text: "Your settings file could not be read, so the defaults are in use. Changing a setting here will replace it.",
				onDismiss: () => dismiss("settings-file"),
			});
		} else if (startup.replaced_fields.length > 0) {
			notices.push({
				id: "settings-file",
				tone: "warning",
				text: `Some settings were invalid and were reset: ${startup.replaced_fields.join(", ")}.`,
				onDismiss: () => dismiss("settings-file"),
			});
		}
	}
	if (startup.unusable_pack !== null && !dismissed.has("saved-pack")) {
		notices.push({
			id: "saved-pack",
			tone: "warning",
			text: `The saved sound pack "${startup.unusable_pack}" could not be used. Playing ${view.fallbackName} instead.`,
			onDismiss: () => dismiss("saved-pack"),
		});
	}
	return notices;
}

function Settings({
	view,
	setUpPermission,
}: {
	view: Ready;
	setUpPermission: () => void;
}) {
	// Start-up notices the user has closed stay closed while the window is open.
	const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
	const dismiss = (id: string) =>
		setDismissed((before) => new Set(before).add(id));

	return (
		<>
			<Notices
				notices={noticesFor(view, dismissed, dismiss, setUpPermission)}
			/>
			<section className="card">
				<SoundSwitch enabled={view.enabled} onChange={view.setEnabled} />
				<VolumeSlider
					percent={view.volumePercent}
					onChange={view.setVolumePercent}
					onRelease={view.flushVolume}
				/>
				<PitchSlider
					percent={view.pitchPercent}
					onChange={view.setPitchPercent}
					onRelease={view.flushPitch}
				/>
				<KeyUpSwitch
					on={view.keyUpSounds}
					note={view.keyUpNote}
					onChange={view.setKeyUpSounds}
				/>
			</section>
			<PackList
				packs={view.packs}
				selected={view.selectedPack}
				onPreview={view.preview}
				importing={view.importing}
				onImport={() => {
					// An imported pack is chosen too, which answers the
					// notice about the saved one.
					void view.importPack().then((imported) => {
						if (imported) {
							dismiss("saved-pack");
						}
					});
				}}
				onRemove={view.removePack}
				onChoose={(id) => {
					// Choosing a pack answers the notice about the saved one.
					dismiss("saved-pack");
					view.choosePack(id);
				}}
			/>
			<LaunchSwitch on={view.launchAtLogin} onChange={view.setLaunchAtLogin} />
			{/*
			 * Read out by a screen reader when a pack is imported or removed.
			 * Not `role="status"`: the warning notices are that. The key makes
			 * the same words a new node, so they are read a second time.
			 */}
			<div
				id="pack-announcement"
				className="visually-hidden"
				aria-live="polite"
			>
				{view.announcement && (
					<span key={view.announcement.count}>{view.announcement.text}</span>
				)}
			</div>
		</>
	);
}

/**
 * The loaded window: the permission screen while macOS keeps the app from
 * hearing keys, otherwise the settings.
 */
function Loaded({ view }: { view: Ready }) {
	// "Set up later" lasts while the window does; it is never saved.
	const [later, setLater] = useState(false);
	const onScreen = needsPermission(view) && !later;

	// When one view gives way to the other, what had focus is gone. It goes
	// to the start of the new view, so a keyboard or screen-reader user is
	// told of the change. Not on the first view: nothing was lost then.
	const shown = useRef(onScreen);
	useEffect(() => {
		if (shown.current === onScreen) {
			return;
		}
		shown.current = onScreen;
		document
			.getElementById(onScreen ? PERMISSION_HEADING_ID : SOUND_SWITCH_ID)
			?.focus();
	}, [onScreen]);

	if (onScreen) {
		return (
			<PermissionScreen
				restart={view.permission === "restart_needed"}
				onOpen={view.openPermissionSettings}
				onLater={() => setLater(true)}
				onRestart={view.restartApp}
			/>
		);
	}
	return <Settings view={view} setUpPermission={() => setLater(false)} />;
}

export function App() {
	const view = useSettings();
	return (
		<main className="window">
			<h1 className="title">Mechanical Key Sounds</h1>
			{view.phase === "ready" && <Loaded view={view} />}
			{view.phase === "error" && (
				<div className="load-error">
					<p role="alert">Settings could not be loaded.</p>
					<button type="button" className="button" onClick={view.retry}>
						Retry
					</button>
				</div>
			)}
		</main>
	);
}
