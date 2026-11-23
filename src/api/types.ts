// The shapes the core's commands take and return. Field names are the
// settings file's, so they are snake_case.

export interface Settings {
	enabled: boolean;
	pack_id: string;
	/** 0 to 1. */
	volume: number;
	/** 0 to 0.3: how far each sound's speed may stray from normal. */
	pitch_variation: number;
	key_up_sounds: boolean;
	launch_at_login: boolean;
}

export interface PackInfo {
	id: string;
	name: string;
	author: string | null;
	license: string;
	/** Whether the pack has a sound for any key going up. */
	has_key_up: boolean;
	/** Whether the user imported it; false for the packs the app ships. */
	imported: boolean;
}

/** What went wrong when the app started, if anything. */
export interface StartupProblems {
	settings_unreadable: boolean;
	replaced_fields: string[];
	/** The saved pack id that could not be used. */
	unusable_pack: string | null;
}

export type AudioStatus = "playing" | "no_device" | "failed";

/**
 * The macOS key-listening permission (Input Monitoring). `not_needed` on
 * every other platform; `restart_needed` when macOS has granted it but the
 * key listener could not start in this run.
 */
export type PermissionState =
	| "not_needed"
	| "needed"
	| "granted"
	| "restart_needed";

/** What the window opens with. */
export interface SettingsState {
	settings: Settings;
	/** The pack actually playing; null for the built-in click. */
	playing_pack: string | null;
	startup: StartupProblems;
	audio: AudioStatus;
	permission: PermissionState;
	/** How many changes the core has applied; see `SetResult.revision`. */
	revision: number;
}

/** The fields this window can change. */
export type SettingsPatch = Partial<
	Pick<
		Settings,
		"enabled" | "volume" | "pack_id" | "pitch_variation" | "key_up_sounds"
	>
>;

export interface SetResult {
	settings: Settings;
	playing_pack: string | null;
	/** False when the change is in force but could not be written. */
	saved: boolean;
	save_error: string | null;
	/**
	 * Rises by one with every change the core applies. A reply and an event
	 * can arrive in either order; the one with the lower revision is old.
	 */
	revision: number;
}

/** A pack that was imported and is now selected. */
export interface ImportResult {
	pack: PackInfo;
	/** Whether an earlier imported pack with the same id was replaced. */
	replaced: boolean;
	/** The whole pack list after the import. */
	packs: PackInfo[];
	/** The change that selected the pack, as `setSettings` returns it. */
	change: SetResult;
}

/** What is left after an imported pack was removed. */
export interface RemoveResult {
	/** The whole pack list after the removal. */
	packs: PackInfo[];
	/** The change to the default pack, if the removed one was in use. */
	change: SetResult | null;
}

/** Stops a subscription. */
export type Unsubscribe = () => void;

/** The core, as the window sees it. A rejection carries a string. */
export interface CoreApi {
	listPacks(): Promise<PackInfo[]>;
	getSettings(): Promise<SettingsState>;
	setSettings(patch: SettingsPatch): Promise<SetResult>;
	audioStatus(): Promise<AudioStatus>;
	/**
	 * Plays a short typed phrase with the sounds in force, even when sound
	 * is off. Resolves once it has started; changes no setting.
	 */
	preview(): Promise<void>;
	/**
	 * Opens the system's folder picker and imports the folder chosen as a
	 * pack, which becomes the selected one. Resolves with null if the picker
	 * was cancelled. Rejects with the whole notice text.
	 */
	importPack(): Promise<ImportResult | null>;
	/** Removes an imported pack. Rejects with the reason. */
	removePack(id: string): Promise<RemoveResult>;
	permissionStatus(): Promise<PermissionState>;
	/**
	 * Asks macOS for the permission and opens System Settings on the Input
	 * Monitoring pane. Rejects if the pane could not be opened.
	 */
	openPermissionSettings(): Promise<void>;
	/** Quits the app and starts it again. */
	restartApp(): Promise<void>;
	/**
	 * Calls `listener` with the result of every change the core applies,
	 * whoever asked for it: this window or the tray.
	 */
	onSettingsChanged(
		listener: (result: SetResult) => void,
	): Promise<Unsubscribe>;
}
