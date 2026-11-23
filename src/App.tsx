import { useState } from "react";
import { type Notice, Notices } from "./components/Notices";
import { PackList } from "./components/PackList";
import { SoundSwitch } from "./components/SoundSwitch";
import { VolumeSlider } from "./components/VolumeSlider";
import { type Ready, useSettings } from "./useSettings";

/** The notices to show, worst first. */
function noticesFor(
	view: Ready,
	dismissed: ReadonlySet<string>,
	dismiss: (id: string) => void,
): Notice[] {
	const notices: Notice[] = [];
	const { startup } = view;

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

function Settings({ view }: { view: Ready }) {
	// Start-up notices the user has closed stay closed while the window is open.
	const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
	const dismiss = (id: string) =>
		setDismissed((before) => new Set(before).add(id));

	return (
		<>
			<Notices notices={noticesFor(view, dismissed, dismiss)} />
			<section className="card">
				<SoundSwitch enabled={view.enabled} onChange={view.setEnabled} />
				<VolumeSlider
					percent={view.volumePercent}
					onChange={view.setVolumePercent}
					onRelease={view.flushVolume}
				/>
			</section>
			<PackList
				packs={view.packs}
				selected={view.selectedPack}
				onChoose={(id) => {
					// Choosing a pack answers the notice about the saved one.
					dismiss("saved-pack");
					view.choosePack(id);
				}}
			/>
		</>
	);
}

export function App() {
	const view = useSettings();
	return (
		<main className="window">
			<h1 className="title">Mechanical Key Sounds</h1>
			{view.phase === "ready" && <Settings view={view} />}
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
