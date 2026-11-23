// The real core: Tauri commands in `src-tauri/src/commands/`.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
	AudioStatus,
	CoreApi,
	PackInfo,
	PermissionState,
	SetResult,
	SettingsState,
} from "./types";

export const tauriCore: CoreApi = {
	listPacks: () => invoke<PackInfo[]>("list_packs"),
	getSettings: () => invoke<SettingsState>("get_settings"),
	setSettings: (patch) => invoke<SetResult>("set_settings", { patch }),
	audioStatus: () => invoke<AudioStatus>("audio_status"),
	permissionStatus: () => invoke<PermissionState>("permission_status"),
	openPermissionSettings: async () => {
		await invoke<boolean>("open_permission_settings");
	},
	restartApp: async () => {
		await invoke<boolean>("restart_app");
	},
	onSettingsChanged: (listener) =>
		listen<SetResult>("settings-changed", (event) => listener(event.payload)),
};
