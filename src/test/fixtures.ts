// Shared by the settings window tests: state for the mocked `src/api`.
import { vi } from "vitest";
import * as api from "../api";
import type {
	AudioStatus,
	PackInfo,
	PermissionState,
	SetResult,
	SettingsState,
} from "../api/types";

export const PACKS: PackInfo[] = [
	{
		id: "blue",
		name: "Blue switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
		has_key_up: true,
		imported: false,
	},
	{
		id: "brown",
		name: "Brown switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
		has_key_up: true,
		imported: false,
	},
	{
		id: "red",
		name: "Red switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
		has_key_up: true,
		imported: false,
	},
	{
		id: "typewriter",
		name: "Typewriter",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
		has_key_up: false,
		imported: false,
	},
];

/** An imported pack, as the core lists it. */
export function importedPack(
	id: string,
	name: string,
	over: Partial<PackInfo> = {},
): PackInfo {
	return {
		id,
		name,
		author: null,
		license: "Licence not stated",
		has_key_up: true,
		imported: true,
		...over,
	};
}

export function state(over: Partial<SettingsState> = {}): SettingsState {
	return {
		settings: {
			enabled: true,
			pack_id: "red",
			volume: 0.8,
			pitch_variation: 0,
			key_up_sounds: true,
			launch_at_login: false,
		},
		playing_pack: "red",
		startup: {
			settings_unreadable: false,
			replaced_fields: [],
			unusable_pack: null,
		},
		audio: "playing",
		permission: "not_needed",
		revision: 0,
		...over,
	};
}

export function result(
	current: SettingsState,
	patch: Record<string, unknown>,
	extra: Partial<SetResult> = {},
): SetResult {
	const settings = { ...current.settings, ...patch };
	return {
		settings,
		playing_pack: settings.pack_id,
		saved: true,
		save_error: null,
		revision: current.revision + 1,
		...extra,
	};
}

/** Resolves each call like a healthy core, keeping its own copy of the state. */
export function healthyCore(initial: SettingsState = state(), packs = PACKS) {
	let current = initial;
	const listeners = new Set<(r: SetResult) => void>();
	vi.mocked(api.onSettingsChanged).mockImplementation(async (listener) => {
		listeners.add(listener);
		return () => {
			listeners.delete(listener);
		};
	});
	vi.mocked(api.getSettings).mockImplementation(async () => current);
	vi.mocked(api.listPacks).mockImplementation(async () => packs);
	vi.mocked(api.audioStatus).mockImplementation(
		async () => current.audio as AudioStatus,
	);
	vi.mocked(api.permissionStatus).mockImplementation(
		async () => current.permission as PermissionState,
	);
	vi.mocked(api.openPermissionSettings).mockResolvedValue(undefined);
	vi.mocked(api.restartApp).mockResolvedValue(undefined);
	vi.mocked(api.preview).mockResolvedValue(undefined);
	vi.mocked(api.importPack).mockResolvedValue(null);
	vi.mocked(api.setSettings).mockImplementation(async (patch) => {
		const r = result(current, patch);
		current = {
			...current,
			settings: r.settings,
			playing_pack: r.playing_pack,
			revision: r.revision,
		};
		return r;
	});
	return {
		setAudio(audio: AudioStatus) {
			current = { ...current, audio };
		},
		/** What `permissionStatus` answers from now on. */
		setPermission(permission: PermissionState) {
			current = { ...current, permission };
		},
		/** How many listeners are subscribed through `onSettingsChanged`. */
		listenerCount: () => listeners.size,
		/**
		 * The core changing the settings outside the window, as the tray does:
		 * it takes the next revision, and every subscribed listener is told.
		 */
		outsideChange(
			patch: Record<string, unknown>,
			extra: Partial<SetResult> = {},
		) {
			const r = result(current, patch, extra);
			current = {
				...current,
				settings: r.settings,
				playing_pack: r.playing_pack,
				revision: r.revision,
			};
			for (const listener of [...listeners]) {
				listener(r);
			}
			return r;
		},
		/** Tells the listeners a result as it is, without changing the core. */
		emit(r: SetResult) {
			for (const listener of [...listeners]) {
				listener(r);
			}
		},
	};
}
