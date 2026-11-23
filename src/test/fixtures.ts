// Shared by the settings window tests: state for the mocked `src/api`.
import { vi } from "vitest";
import * as api from "../api";
import type {
	AudioStatus,
	PackInfo,
	SetResult,
	SettingsState,
} from "../api/types";

export const PACKS: PackInfo[] = [
	{
		id: "blue",
		name: "Blue switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
	},
	{
		id: "brown",
		name: "Brown switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
	},
	{
		id: "red",
		name: "Red switches",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
	},
	{
		id: "typewriter",
		name: "Typewriter",
		author: "Mechanical Key Sounds contributors",
		license: "CC0-1.0",
	},
];

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
		...extra,
	};
}

/** Resolves each call like a healthy core, keeping its own copy of the state. */
export function healthyCore(initial: SettingsState = state(), packs = PACKS) {
	let current = initial;
	vi.mocked(api.getSettings).mockImplementation(async () => current);
	vi.mocked(api.listPacks).mockImplementation(async () => packs);
	vi.mocked(api.audioStatus).mockImplementation(
		async () => current.audio as AudioStatus,
	);
	vi.mocked(api.setSettings).mockImplementation(async (patch) => {
		const r = result(current, patch);
		current = {
			...current,
			settings: r.settings,
			playing_pack: r.playing_pack,
		};
		return r;
	});
	return {
		setAudio(audio: AudioStatus) {
			current = { ...current, audio };
		},
	};
}
