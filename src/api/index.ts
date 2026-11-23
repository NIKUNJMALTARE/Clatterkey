// The only way the UI reaches the core. Tests replace this module.
import { tauriCore } from "./tauri";
import type {
	AudioStatus,
	CoreApi,
	ImportResult,
	PackInfo,
	PermissionState,
	RemoveResult,
	SetResult,
	SettingsPatch,
	SettingsState,
	Unsubscribe,
} from "./types";

let chosen: Promise<CoreApi> | undefined;

/**
 * The Tauri core inside the app. In a plain browser during development
 * there is no core, so an in-memory fake answers; it is loaded only then
 * and is not part of the production bundle.
 */
function core(): Promise<CoreApi> {
	chosen ??= choose();
	return chosen;
}

async function choose(): Promise<CoreApi> {
	if ("__TAURI_INTERNALS__" in window) {
		return tauriCore;
	}
	if (import.meta.env.DEV) {
		const { createFakeCore } = await import("./fake");
		return createFakeCore(window.location.search);
	}
	return tauriCore;
}

export async function listPacks(): Promise<PackInfo[]> {
	return (await core()).listPacks();
}

export async function getSettings(): Promise<SettingsState> {
	return (await core()).getSettings();
}

export async function setSettings(patch: SettingsPatch): Promise<SetResult> {
	return (await core()).setSettings(patch);
}

export async function audioStatus(): Promise<AudioStatus> {
	return (await core()).audioStatus();
}

export async function preview(): Promise<void> {
	return (await core()).preview();
}

export async function importPack(): Promise<ImportResult | null> {
	return (await core()).importPack();
}

export async function removePack(id: string): Promise<RemoveResult> {
	return (await core()).removePack(id);
}

export async function permissionStatus(): Promise<PermissionState> {
	return (await core()).permissionStatus();
}

export async function openPermissionSettings(): Promise<void> {
	return (await core()).openPermissionSettings();
}

export async function restartApp(): Promise<void> {
	return (await core()).restartApp();
}

export async function onSettingsChanged(
	listener: (result: SetResult) => void,
): Promise<Unsubscribe> {
	return (await core()).onSettingsChanged(listener);
}
