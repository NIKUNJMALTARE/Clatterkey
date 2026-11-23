// The settings window's state: what the core last confirmed, what the user
// has asked for since, and the notices. Components only render it.
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./api";
import type {
	AudioStatus,
	PackInfo,
	PermissionState,
	SetResult,
	Settings,
	SettingsPatch,
	StartupProblems,
} from "./api/types";

/** How often a moving slider's value is sent, at most. */
const VOLUME_INTERVAL_MS = 100;
/** How often the audio output's status is read. */
const AUDIO_POLL_MS = 1000;
/** How often the macOS permission's state is read, where there is one. */
const PERMISSION_POLL_MS = 1000;

type Key = keyof SettingsPatch;
type Change = { [K in Key]: { key: K; value: Settings[K] } }[Key];

/** What the core last said is in force. */
interface Confirmed {
	settings: Settings;
	playing: string | null;
}

export interface Ready {
	phase: "ready";
	packs: PackInfo[];
	enabled: boolean;
	/** 0 to 100. */
	volumePercent: number;
	/** The selected pack's id; null when the built-in click is playing. */
	selectedPack: string | null;
	audio: AudioStatus;
	/** The macOS key-listening permission, as the core last reported it. */
	permission: PermissionState;
	startup: StartupProblems;
	/** The name of what played when the saved pack could not be used. */
	fallbackName: string;
	/** Why the last chosen pack could not be loaded, and its name. */
	packError: { name: string; reason: string } | null;
	/** Why the last save failed. */
	saveError: string | null;
	/** Why the last change to the switch or the volume was refused. */
	changeError: string | null;
	setEnabled(enabled: boolean): void;
	setVolumePercent(percent: number): void;
	/** Sends a volume that is waiting for the throttle, now. */
	flushVolume(): void;
	choosePack(id: string): void;
	dismissPackError(): void;
	dismissChangeError(): void;
	/** Rejects if System Settings could not be opened. */
	openPermissionSettings(): Promise<void>;
	restartApp(): void;
}

export type SettingsView =
	| { phase: "loading" }
	| { phase: "error"; retry(): void }
	| Ready;

function reasonOf(error: unknown): string {
	if (typeof error === "string") {
		return error;
	}
	if (error instanceof Error) {
		return error.message;
	}
	return "unknown error";
}

function isAudioStatus(value: unknown): value is AudioStatus {
	return value === "playing" || value === "no_device" || value === "failed";
}

function isPermissionState(value: unknown): value is PermissionState {
	return (
		value === "not_needed" ||
		value === "needed" ||
		value === "granted" ||
		value === "restart_needed"
	);
}

export function useSettings(): SettingsView {
	const [phase, setPhase] = useState<"loading" | "error" | "ready">("loading");
	const [attempt, setAttempt] = useState(0);
	const [packs, setPacks] = useState<PackInfo[]>([]);
	const [confirmed, setConfirmed] = useState<Confirmed | null>(null);
	const [startup, setStartup] = useState<StartupProblems | null>(null);
	const [fallbackName, setFallbackName] = useState("");
	const [audio, setAudio] = useState<AudioStatus>("playing");
	const [permission, setPermission] = useState<PermissionState>("not_needed");
	// What the user has asked for and the core has not yet answered. It is
	// shown over `confirmed`, so a control never jumps back while waiting.
	const [desired, setDesired] = useState<SettingsPatch>({});
	const [packError, setPackError] = useState<Ready["packError"]>(null);
	const [saveError, setSaveError] = useState<string | null>(null);
	const [changeError, setChangeError] = useState<string | null>(null);

	const alive = useRef(true);
	// The revision of what `confirmed` holds. Replies, events from the core
	// and re-reads can arrive in any order; an older one is dropped.
	const revision = useRef(0);
	// Whether the first read has arrived, the listening that the read waits
	// for, and a change that came in before the read did.
	const loaded = useRef(false);
	const subscribed = useRef<Promise<void> | null>(null);
	const waiting = useRef<SetResult | null>(null);
	const packsRef = useRef<PackInfo[]>([]);
	// Changes not yet sent, one per field, oldest first. They go to the core
	// one at a time, so its answers arrive in the order of the requests.
	const queue = useRef<Change[]>([]);
	const sending = useRef(false);
	const volumeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const volumeWaiting = useRef<number | null>(null);

	useEffect(() => {
		alive.current = true;
		return () => {
			alive.current = false;
		};
	}, []);

	/**
	 * Takes what the core says is in force, unless something newer has
	 * already been taken. Returns whether it was taken.
	 */
	const confirm = useCallback(
		(result: Pick<SetResult, "settings" | "playing_pack" | "revision">) => {
			if (!alive.current || !result?.settings) {
				return false;
			}
			const at = result.revision ?? revision.current;
			if (at < revision.current) {
				return false;
			}
			revision.current = at;
			setConfirmed({
				settings: result.settings,
				playing: result.playing_pack,
			});
			return true;
		},
		[],
	);

	/** A change the core applied, asked for by this window or by the tray. */
	const confirmChange = useCallback(
		(result: SetResult) => {
			if (confirm(result)) {
				setSaveError(
					result.saved ? null : (result.save_error ?? "unknown error"),
				);
			}
		},
		[confirm],
	);

	// Changes made outside the window (the tray) reach it as events. They
	// are only shown: nothing is sent back to the core. This effect comes
	// before the first read so that the read can wait for it.
	useEffect(() => {
		let stopped = false;
		let unsubscribe: (() => void) | undefined;
		const listener = (result: SetResult) => {
			if (loaded.current) {
				confirmChange(result);
			} else if (
				!waiting.current ||
				(result?.revision ?? 0) >= (waiting.current.revision ?? 0)
			) {
				// Kept until the first read has arrived.
				waiting.current = result;
			}
		};
		// A failed subscription is ignored: the window works without it.
		const done = Promise.resolve()
			.then(() => api.onSettingsChanged(listener))
			.then((stop) => {
				if (typeof stop !== "function") {
					return;
				}
				if (stopped) {
					stop();
				} else {
					unsubscribe = stop;
				}
			})
			.catch(() => {});
		subscribed.current = done;
		return () => {
			stopped = true;
			unsubscribe?.();
			if (subscribed.current === done) {
				subscribed.current = null;
			}
		};
	}, [confirmChange]);

	// The first read, and each retry.
	// biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` is the trigger for a retry.
	useEffect(() => {
		let current = true;
		setPhase("loading");
		loaded.current = false;
		// Listening starts before the read, so a change the core applies
		// after the read is not missed; one applied before it is in the read.
		(subscribed.current ?? Promise.resolve())
			.then(() => Promise.all([api.getSettings(), api.listPacks()]))
			.then(([state, list]) => {
				if (!current) {
					return;
				}
				if (!state?.settings || !Array.isArray(list)) {
					setPhase("error");
					return;
				}
				packsRef.current = list;
				setPacks(list);
				revision.current = state.revision ?? 0;
				setConfirmed({ settings: state.settings, playing: state.playing_pack });
				setStartup(state.startup);
				setFallbackName(
					list.find((p) => p.id === state.playing_pack)?.name ??
						"the built-in click",
				);
				setAudio(state.audio);
				// A core that does not say is one with no permission to give.
				setPermission(
					isPermissionState(state.permission) ? state.permission : "not_needed",
				);
				setPhase("ready");
				loaded.current = true;
				// A change that arrived during the read; dropped if the read
				// already had it.
				const early = waiting.current;
				waiting.current = null;
				const at = early?.revision ?? 0;
				if (early && at > revision.current) {
					confirmChange(early);
				} else if (early && at === revision.current && !early.saved) {
					// The read already has this change, but only the event
					// says that saving it failed.
					setSaveError(early.save_error ?? "unknown error");
				}
			})
			.catch(() => {
				if (current) {
					setPhase("error");
				}
			});
		return () => {
			current = false;
		};
	}, [attempt]);

	// The audio notices come and go while the window is open.
	useEffect(() => {
		if (phase !== "ready") {
			return;
		}
		const timer = setInterval(() => {
			Promise.resolve()
				.then(() => api.audioStatus())
				.then((status) => {
					if (alive.current && isAudioStatus(status)) {
						setAudio(status);
					}
				})
				.catch(() => {});
		}, AUDIO_POLL_MS);
		return () => clearInterval(timer);
	}, [phase]);

	// The permission is given, and can be taken away, in System Settings
	// while the window is open. Where there is none, nothing is asked.
	const pollPermission = phase === "ready" && permission !== "not_needed";
	useEffect(() => {
		if (!pollPermission) {
			return;
		}
		const timer = setInterval(() => {
			Promise.resolve()
				.then(() => api.permissionStatus())
				.then((status) => {
					if (alive.current && isPermissionState(status)) {
						setPermission(status);
					}
				})
				.catch(() => {});
		}, PERMISSION_POLL_MS);
		return () => clearInterval(timer);
	}, [pollPermission]);

	// A hidden window may miss an event, so when it is shown again or gains
	// focus the settings are read again, quietly: no loading state, and a
	// failure changes nothing.
	useEffect(() => {
		if (phase !== "ready") {
			return;
		}
		const reread = () => {
			if (document.visibilityState === "hidden") {
				return;
			}
			Promise.resolve()
				.then(() => api.getSettings())
				.then((state) => {
					confirm(state);
					if (alive.current && isPermissionState(state?.permission)) {
						setPermission(state.permission);
					}
				})
				.catch(() => {});
		};
		window.addEventListener("focus", reread);
		document.addEventListener("visibilitychange", reread);
		return () => {
			window.removeEventListener("focus", reread);
			document.removeEventListener("visibilitychange", reread);
		};
	}, [phase, confirm]);

	const send = useCallback(async () => {
		if (sending.current) {
			return;
		}
		sending.current = true;
		try {
			for (
				let change = queue.current.shift();
				change;
				change = queue.current.shift()
			) {
				const { key, value } = change;
				try {
					const result = await api.setSettings({ [key]: value });
					if (!alive.current) {
						return;
					}
					confirmChange(result);
					if (key === "pack_id") {
						// A pack has loaded since: an earlier failure is old news.
						setPackError(null);
					}
				} catch (error) {
					if (!alive.current) {
						return;
					}
					const reason = reasonOf(error);
					if (key === "pack_id") {
						const name =
							packsRef.current.find((p) => p.id === value)?.name ??
							String(value);
						setPackError({ name, reason });
					} else {
						setChangeError(reason);
					}
				}
				// With no newer request for this field, show what the core
				// confirmed: the new value, or the old one after a refusal.
				// A volume still waiting for the throttle is a newer request
				// too: without this the slider would jump back mid-drag.
				const newer =
					queue.current.some((later) => later.key === key) ||
					(key === "volume" && volumeWaiting.current !== null);
				if (!newer) {
					setDesired((d) => {
						const { [key]: _answered, ...rest } = d;
						return rest;
					});
				}
			}
		} finally {
			sending.current = false;
		}
	}, [confirmChange]);

	const change = useCallback(
		(next: Change) => {
			setDesired((d) => ({ ...d, [next.key]: next.value }));
			const waiting = queue.current.findIndex((c) => c.key === next.key);
			if (waiting >= 0) {
				queue.current[waiting] = next;
			} else {
				queue.current.push(next);
			}
			void send();
		},
		[send],
	);

	const flushVolume = useCallback(() => {
		if (volumeTimer.current !== null) {
			clearTimeout(volumeTimer.current);
			volumeTimer.current = null;
		}
		const volume = volumeWaiting.current;
		volumeWaiting.current = null;
		if (volume !== null) {
			change({ key: "volume", value: volume });
		}
	}, [change]);

	const setVolumePercent = useCallback(
		(percent: number) => {
			const volume = Math.min(100, Math.max(0, Math.round(percent))) / 100;
			setDesired((d) => ({ ...d, volume }));
			volumeWaiting.current = volume;
			// The timer is not restarted by later movements, so while the
			// slider is dragged the core hears of it every 100 ms, and the
			// last value is always sent.
			volumeTimer.current ??= setTimeout(flushVolume, VOLUME_INTERVAL_MS);
		},
		[flushVolume],
	);

	// A value still waiting when the window goes is sent, not lost.
	useEffect(() => flushVolume, [flushVolume]);

	const retry = useCallback(() => setAttempt((n) => n + 1), []);

	if (phase === "loading") {
		return { phase };
	}
	if (phase === "error" || !confirmed || !startup) {
		return { phase: "error", retry };
	}

	const volume = desired.volume ?? confirmed.settings.volume;
	return {
		phase: "ready",
		packs,
		enabled: desired.enabled ?? confirmed.settings.enabled,
		volumePercent: Math.round(volume * 100),
		selectedPack: desired.pack_id ?? confirmed.playing,
		audio,
		permission,
		startup,
		fallbackName,
		packError,
		saveError,
		changeError,
		setEnabled: (enabled) => change({ key: "enabled", value: enabled }),
		setVolumePercent,
		flushVolume,
		choosePack: (id) => {
			setPackError(null);
			change({ key: "pack_id", value: id });
		},
		dismissPackError: () => setPackError(null),
		dismissChangeError: () => setChangeError(null),
		openPermissionSettings: () =>
			Promise.resolve().then(() => api.openPermissionSettings()),
		restartApp: () => {
			// If the restart fails the screen stays, with its button.
			Promise.resolve()
				.then(() => api.restartApp())
				.catch(() => {});
		},
	};
}
