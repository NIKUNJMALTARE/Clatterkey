// An in-memory core for viewing the window in a plain browser during
// development, where there is no Tauri. `?state=` picks what it shows.
// Loaded only in development builds; see `index.ts`.
import type {
	CoreApi,
	ImportResult,
	PackInfo,
	PermissionState,
	SetResult,
	Settings,
	SettingsPatch,
	SettingsState,
} from "./types";

/**
 * Stands in for the tray in a plain browser: `window.__mksFake` changes the
 * settings from outside the window, as the tray's "Sound" item does, and
 * moves the macOS permission between its states, as System Settings does.
 */
export interface FakeControls {
	outsideChange(patch: SettingsPatch, options?: { saveFails?: boolean }): void;
	setPermission(permission: PermissionState): void;
}

declare global {
	interface Window {
		__mksFake?: FakeControls;
	}
}

/** Only the fake contains this text; the build is checked for its absence. */
export const FAKE_MARKER = "mks-dev-fake-core";

const AUTHOR = "Mechanical Key Sounds contributors";

const pack = (id: string, name: string, has_key_up = true): PackInfo => ({
	id,
	name,
	author: AUTHOR,
	license: "CC0-1.0",
	has_key_up,
	imported: false,
});

/** What the fake's import brings in: a plain folder wrapped as a pack. */
const MY_CLICKS: PackInfo = {
	id: "my-clicks",
	name: "My clicks",
	author: null,
	license: "Licence not stated",
	has_key_up: true,
	imported: true,
};

/** An imported pack with the longest name a pack can have: 60 characters. */
const LONG_NAME: PackInfo = {
	id: "grandfathers-typewriter",
	name: "Grandfather's typewriter, recorded in the attic, second take",
	author: "A. N. Other",
	license: "CC-BY-4.0",
	has_key_up: false,
	imported: true,
};

const byName = (a: PackInfo, b: PackInfo) =>
	a.name.toLowerCase().localeCompare(b.name.toLowerCase());

const PACKS: PackInfo[] = [
	pack("blue", "Blue switches"),
	pack("brown", "Brown switches"),
	pack("red", "Red switches"),
	pack("typewriter", "Typewriter", false),
];

const STATES = [
	"normal",
	"loading",
	"load-error",
	"no-audio",
	"audio-error",
	"settings-invalid",
	"fields-reset",
	"pack-fallback",
	"pack-load-error",
	"save-error",
	"no-packs",
	"permission-needed",
	"permission-restart",
	"permission-open-fails",
	"no-key-up",
	"preview-error",
	"imported",
	"import-error",
	"remove-error",
] as const;

type FakeState = (typeof STATES)[number];

function stateFrom(search: string): FakeState {
	const asked = new URLSearchParams(search).get("state");
	return STATES.find((s) => s === asked) ?? "normal";
}

const later = <T>(value: T): Promise<T> =>
	new Promise((resolve) => setTimeout(() => resolve(value), 30));

export function createFakeCore(search: string): CoreApi {
	const state = stateFrom(search);
	console.info(`${FAKE_MARKER}: no Tauri core, showing state "${state}"`);

	const startsImported = state === "imported" || state === "remove-error";
	let packs: PackInfo[] =
		state === "no-packs"
			? []
			: startsImported
				? [...PACKS, MY_CLICKS, LONG_NAME].sort(byName)
				: PACKS;
	// The pack playing at the start; Typewriter has no key-up sounds.
	const first = state === "no-key-up" ? "typewriter" : "brown";
	const settings: Settings = {
		enabled: true,
		pack_id: state === "pack-fallback" ? "nope" : first,
		volume: 0.8,
		pitch_variation: 0,
		key_up_sounds: true,
		launch_at_login: false,
	};
	let playing: string | null = state === "no-packs" ? null : first;
	let revision = 0;
	// Every state but the permission ones is a platform with no permission.
	let permission: PermissionState =
		state === "permission-needed" || state === "permission-open-fails"
			? "needed"
			: state === "permission-restart"
				? "restart_needed"
				: "not_needed";
	const listeners = new Set<(result: SetResult) => void>();

	/** Applies a change as the core does and tells every listener. */
	const applied = (patch: SettingsPatch, failed: boolean): SetResult => {
		Object.assign(settings, patch);
		if (patch.pack_id !== undefined) {
			playing = patch.pack_id;
		}
		revision += 1;
		const result: SetResult = {
			settings: { ...settings },
			playing_pack: playing,
			saved: !failed,
			save_error: failed ? "Permission denied (os error 13)" : null,
			revision,
		};
		for (const listener of [...listeners]) {
			listener(result);
		}
		return result;
	};

	window.__mksFake = {
		outsideChange: (patch, options) => {
			applied(patch, options?.saveFails ?? false);
		},
		setPermission: (next) => {
			permission = next;
		},
	};

	const opening = (): SettingsState => ({
		settings: { ...settings },
		playing_pack: playing,
		startup: {
			settings_unreadable: state === "settings-invalid",
			replaced_fields:
				state === "fields-reset" ? ["volume", "key_up_sounds"] : [],
			unusable_pack: state === "pack-fallback" ? "nope" : null,
		},
		audio:
			state === "no-audio"
				? "no_device"
				: state === "audio-error"
					? "failed"
					: "playing",
		permission,
		revision,
	});

	return {
		listPacks: () => later([...packs]),
		getSettings: () => {
			if (state === "loading") {
				return new Promise<SettingsState>(() => {});
			}
			if (state === "load-error") {
				return Promise.reject("the core did not answer");
			}
			return later(opening());
		},
		setSettings: async (patch) => {
			await later(null);
			if (patch.pack_id !== undefined && state === "pack-load-error") {
				throw `pack "${patch.pack_id}": keys.default[0] "down/default-1.wav": file not found`;
			}
			return applied(patch, state === "save-error");
		},
		audioStatus: () => later(opening().audio),
		preview: async () => {
			await later(null);
			if (state === "preview-error") {
				throw "the preview could not be started: Resource temporarily unavailable (os error 35)";
			}
		},
		importPack: async (): Promise<ImportResult | null> => {
			await later(null);
			if (state === "import-error") {
				throw 'Could not import "Old Board": "thock 3.wav": longer than 3 seconds';
			}
			const replaced = packs.some((p) => p.id === MY_CLICKS.id);
			packs = [...packs.filter((p) => p.id !== MY_CLICKS.id), MY_CLICKS].sort(
				byName,
			);
			const change = applied({ pack_id: MY_CLICKS.id }, state === "save-error");
			return { pack: MY_CLICKS, replaced, packs: [...packs], change };
		},
		removePack: async (id) => {
			await later(null);
			const pack = packs.find((p) => p.id === id);
			if (!pack) {
				throw `unknown sound pack "${id}"`;
			}
			if (!pack.imported) {
				throw `"${id}" is a bundled pack and cannot be removed`;
			}
			if (state === "remove-error") {
				throw "Access is denied. (os error 5)";
			}
			packs = packs.filter((p) => p.id !== id);
			const change =
				playing === id || settings.pack_id === id
					? applied({ pack_id: "brown" }, state === "save-error")
					: null;
			return { packs: [...packs], change };
		},
		permissionStatus: () => later(permission),
		openPermissionSettings: async () => {
			await later(null);
			if (state === "permission-open-fails") {
				throw "open ended with exit status: 1";
			}
		},
		restartApp: async () => {
			await later(null);
			// A restarted app comes back with the listener running.
			permission = "granted";
		},
		onSettingsChanged: async (listener) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
	};
}
