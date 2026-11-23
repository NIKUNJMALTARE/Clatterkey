// The real core: Tauri commands in `src-tauri/src/commands/`.
import { invoke } from "@tauri-apps/api/core";
import type {
	AudioStatus,
	CoreApi,
	PackInfo,
	SetResult,
	SettingsState,
} from "./types";

export const tauriCore: CoreApi = {
	listPacks: () => invoke<PackInfo[]>("list_packs"),
	getSettings: () => invoke<SettingsState>("get_settings"),
	setSettings: (patch) => invoke<SetResult>("set_settings", { patch }),
	audioStatus: () => invoke<AudioStatus>("audio_status"),
};
