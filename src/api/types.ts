// The shapes the core's commands take and return. Field names are the
// settings file's, so they are snake_case.

export interface Settings {
	enabled: boolean;
	pack_id: string;
	/** 0 to 1. */
	volume: number;
	pitch_variation: number;
	key_up_sounds: boolean;
	launch_at_login: boolean;
}

export interface PackInfo {
	id: string;
	name: string;
	author: string | null;
	license: string;
}

/** What went wrong when the app started, if anything. */
export interface StartupProblems {
	settings_unreadable: boolean;
	replaced_fields: string[];
	/** The saved pack id that could not be used. */
	unusable_pack: string | null;
}

export type AudioStatus = "playing" | "no_device" | "failed";

/** What the window opens with. */
export interface SettingsState {
	settings: Settings;
	/** The pack actually playing; null for the built-in click. */
	playing_pack: string | null;
	startup: StartupProblems;
	audio: AudioStatus;
}

/** The fields this window can change. */
export type SettingsPatch = Partial<
	Pick<Settings, "enabled" | "volume" | "pack_id">
>;

export interface SetResult {
	settings: Settings;
	playing_pack: string | null;
	/** False when the change is in force but could not be written. */
	saved: boolean;
	save_error: string | null;
}

/** The core, as the window sees it. A rejection carries a string. */
export interface CoreApi {
	listPacks(): Promise<PackInfo[]>;
	getSettings(): Promise<SettingsState>;
	setSettings(patch: SettingsPatch): Promise<SetResult>;
	audioStatus(): Promise<AudioStatus>;
}
