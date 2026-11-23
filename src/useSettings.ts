// The settings window's state: what the core last confirmed, what the user
// has asked for since, and the notices. Components only render it.
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./api";
import type {
	AudioStatus,
	PackInfo,
	Settings,
	SettingsPatch,
	StartupProblems,
} from "./api/types";

/** How often a moving slider's value is sent, at most. */
const VOLUME_INTERVAL_MS = 100;
/** How often the audio output's status is read. */
const AUDIO_POLL_MS = 1000;

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

export function useSettings(): SettingsView {
	const [phase, setPhase] = useState<"loading" | "error" | "ready">("loading");
	const [attempt, setAttempt] = useState(0);
	const [packs, setPacks] = useState<PackInfo[]>([]);
	const [confirmed, setConfirmed] = useState<Confirmed | null>(null);
	const [startup, setStartup] = useState<StartupProblems | null>(null);
	const [fallbackName, setFallbackName] = useState("");
	const [audio, setAudio] = useState<AudioStatus>("playing");
	// What the user has asked for and the core has not yet answered. It is
	// shown over `confirmed`, so a control never jumps back while waiting.
	const [desired, setDesired] = useState<SettingsPatch>({});
	const [packError, setPackError] = useState<Ready["packError"]>(null);
	const [saveError, setSaveError] = useState<string | null>(null);
	const [changeError, setChangeError] = useState<string | null>(null);

	const alive = useRef(true);
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

	// The first read, and each retry.
	// biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` is the trigger for a retry.
	useEffect(() => {
		let current = true;
		setPhase("loading");
		Promise.all([api.getSettings(), api.listPacks()])
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
				setConfirmed({ settings: state.settings, playing: state.playing_pack });
				setStartup(state.startup);
				setFallbackName(
					list.find((p) => p.id === state.playing_pack)?.name ??
						"the built-in click",
				);
				setAudio(state.audio);
				setPhase("ready");
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
					setConfirmed({
						settings: result.settings,
						playing: result.playing_pack,
					});
					setSaveError(
						result.saved ? null : (result.save_error ?? "unknown error"),
					);
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
	}, []);

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
	};
}
