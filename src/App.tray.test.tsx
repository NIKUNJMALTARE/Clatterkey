// Feature 07 (tray and background), the window's half: AC12 to AC15. The
// core is mocked at `src/api`; `outsideChange` in the fixtures plays the part
// of a change made from the tray, delivered through `onSettingsChanged`.
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as api from "./api";
import type { SetResult } from "./api/types";
import { healthyCore, result, state } from "./test/fixtures";

vi.mock("./api", () => ({
	listPacks: vi.fn(),
	getSettings: vi.fn(),
	setSettings: vi.fn(),
	audioStatus: vi.fn(),
	permissionStatus: vi.fn(),
	openPermissionSettings: vi.fn(),
	restartApp: vi.fn(),
	onSettingsChanged: vi.fn(),
}));

const mocked = vi.mocked(api);

beforeEach(() => {
	vi.resetAllMocks();
});

async function loaded() {
	render(<App />);
	return screen.findByRole("switch", { name: "Sound" });
}

const NOTICE = /Could not save your settings/;

function deferred<T>() {
	let resolve!: (v: T) => void;
	const promise = new Promise<T>((res) => {
		resolve = res;
	});
	return { promise, resolve };
}

describe("AC12: the switch follows changes made outside the window", () => {
	it("AC12: an outside change to off turns the switch off with no reload and no set_settings call", async () => {
		const core = healthyCore();
		const sw = await loaded();
		expect(sw).toBeChecked();
		await waitFor(() => expect(core.listenerCount()).toBe(1));

		act(() => {
			core.outsideChange({ enabled: false });
		});
		await waitFor(() => expect(sw).not.toBeChecked());
		expect(mocked.setSettings).not.toHaveBeenCalled();
		expect(mocked.getSettings).toHaveBeenCalledTimes(1);
	});

	it("AC12: and the reverse, from off to on", async () => {
		const core = healthyCore(
			state({ settings: { ...state().settings, enabled: false } }),
		);
		const sw = await loaded();
		expect(sw).not.toBeChecked();
		await waitFor(() => expect(core.listenerCount()).toBe(1));

		act(() => {
			core.outsideChange({ enabled: true });
		});
		await waitFor(() => expect(sw).toBeChecked());
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC12: an outside change to the volume moves the slider", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ volume: 0.3 });
		});
		await waitFor(() =>
			expect(screen.getByRole("slider", { name: "Volume" })).toHaveValue("30"),
		);
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC12 edge: a result with a lower revision than the one applied is ignored", async () => {
		const core = healthyCore();
		const sw = await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		const first = core.outsideChange({ enabled: false });
		await waitFor(() => expect(sw).not.toBeChecked());

		act(() => {
			core.emit(
				result(state(), { enabled: true }, { revision: first.revision - 1 }),
			);
		});
		expect(sw).not.toBeChecked();
	});

	it("AC12 edge: it stops listening when the window unmounts", async () => {
		const core = healthyCore();
		const { unmount } = render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		unmount();
		await waitFor(() => expect(core.listenerCount()).toBe(0));
	});

	it("AC12 edge: a failed subscription is ignored and the window still works", async () => {
		healthyCore();
		mocked.onSettingsChanged.mockRejectedValue("no events");
		const sw = await loaded();
		fireEvent.click(sw);
		await waitFor(() => expect(sw).not.toBeChecked());
		expect(mocked.setSettings).toHaveBeenCalledWith({ enabled: false });
	});
});

describe("AC13: an outside change whose save failed", () => {
	it("AC13: the notice shows, and the next successful save removes it", async () => {
		const core = healthyCore();
		const sw = await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));

		act(() => {
			core.outsideChange(
				{ enabled: false },
				{ saved: false, save_error: "disk is full" },
			);
		});
		const notice = await screen.findByText(NOTICE);
		expect(notice).toHaveTextContent("disk is full");
		expect(sw).not.toBeChecked();

		fireEvent.click(sw);
		await waitFor(() => expect(sw).toBeChecked());
		await waitFor(() =>
			expect(screen.queryByText(NOTICE)).not.toBeInTheDocument(),
		);
	});

	it("AC13: a later outside change that saved removes the notice too", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ enabled: false }, { saved: false, save_error: "x" });
		});
		await screen.findByText(NOTICE);
		act(() => {
			core.outsideChange({ enabled: true });
		});
		await waitFor(() =>
			expect(screen.queryByText(NOTICE)).not.toBeInTheDocument(),
		);
	});
});

describe("AC14: the user's own pending change and an outside one", () => {
	it("AC14: the switch keeps the user's value while waiting, then ends on the last value the core confirmed", async () => {
		const core = healthyCore();
		const sw = await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));

		// The user turns it off; the core has not answered yet.
		const reply = deferred<SetResult>();
		mocked.setSettings.mockImplementation(() => reply.promise);
		fireEvent.click(sw);
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		expect(sw).not.toBeChecked();

		// The core applied the user's change (revision 1), then the tray turned
		// the sound back on (revision 2). The event overtakes the reply.
		const own = result(state(), { enabled: false }, { revision: 1 });
		act(() => {
			core.emit(result(state(), { enabled: true }, { revision: 2 }));
		});
		expect(sw).not.toBeChecked(); // no flicker: the pending change stays on top

		await act(async () => {
			reply.resolve(own);
		});
		// The older reply is dropped: the switch ends on revision 2.
		await waitFor(() => expect(sw).toBeChecked());
	});

	it("AC14: when the reply is the newest result the switch ends on it", async () => {
		const core = healthyCore();
		const sw = await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		const reply = deferred<SetResult>();
		mocked.setSettings.mockImplementation(() => reply.promise);
		fireEvent.click(sw);
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));

		act(() => {
			core.emit(result(state(), { enabled: true }, { revision: 1 }));
		});
		expect(sw).not.toBeChecked();
		await act(async () => {
			reply.resolve(result(state(), { enabled: false }, { revision: 2 }));
		});
		await waitFor(() => expect(sw).not.toBeChecked());
		act(() => {
			core.emit(result(state(), { enabled: false }, { revision: 2 }));
		});
		expect(sw).not.toBeChecked();
	});
});

describe("AC15: the window shown again", () => {
	it("AC15: a focus event re-reads the settings quietly and shows a newer revision with no loading or error state", async () => {
		healthyCore();
		const sw = await loaded();
		expect(sw).toBeChecked();
		// The hidden window missed an event: the core is now at revision 5, off.
		mocked.getSettings.mockResolvedValue(
			state({
				settings: { ...state().settings, enabled: false },
				revision: 5,
			}),
		);
		const callsBefore = mocked.getSettings.mock.calls.length;

		fireEvent.focus(window);
		// No loading state: the controls stay on screen at once.
		expect(screen.getByRole("switch", { name: "Sound" })).toBeInTheDocument();
		await waitFor(() =>
			expect(mocked.getSettings.mock.calls.length).toBeGreaterThan(callsBefore),
		);
		await waitFor(() => expect(sw).not.toBeChecked());
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		expect(
			screen.queryByText("Settings could not be loaded."),
		).not.toBeInTheDocument();
	});

	it("AC15: the window becoming visible re-reads the settings too", async () => {
		healthyCore();
		const sw = await loaded();
		mocked.getSettings.mockResolvedValue(
			state({
				settings: { ...state().settings, enabled: false },
				revision: 2,
			}),
		);
		fireEvent(document, new Event("visibilitychange"));
		await waitFor(() => expect(sw).not.toBeChecked());
	});

	it("AC15: an older revision from the re-read is not applied", async () => {
		const core = healthyCore();
		const sw = await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		core.outsideChange({ enabled: false }); // revision 1
		await waitFor(() => expect(sw).not.toBeChecked());
		mocked.getSettings.mockResolvedValue(state({ revision: 0 }));
		fireEvent.focus(window);
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalledTimes(2));
		expect(sw).not.toBeChecked();
	});

	it("AC15: a failed re-read is ignored: no error state, the values stay", async () => {
		healthyCore();
		const sw = await loaded();
		mocked.getSettings.mockRejectedValue("boom");
		fireEvent.focus(window);
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalledTimes(2));
		expect(sw).toBeChecked();
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});
});

describe("AC12: an outside change during start-up is not lost", () => {
	it("AC12: the window subscribes, and the subscription settles, before it reads the settings", async () => {
		healthyCore();
		const log: string[] = [];
		const realGet = mocked.getSettings.getMockImplementation();
		mocked.onSettingsChanged.mockImplementation(async () => {
			log.push("subscribe");
			await new Promise((r) => setTimeout(r, 20));
			log.push("subscribed");
			return () => {};
		});
		mocked.getSettings.mockImplementation(async () => {
			log.push("read");
			return realGet ? realGet() : state();
		});
		await loaded();
		expect(log.slice(0, 3)).toEqual(["subscribe", "subscribed", "read"]);
	});

	it("AC12: a change that arrives while the read is pending is applied when the older read resolves, with no set_settings call", async () => {
		const core = healthyCore();
		const read = deferred<ReturnType<typeof state>>();
		mocked.getSettings.mockImplementation(() => read.promise);
		render(<App />);
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalled());

		act(() => {
			core.emit(result(state(), { enabled: false }, { revision: 1 }));
		});
		await act(async () => {
			read.resolve(state({ revision: 0 }));
		});
		const sw = await screen.findByRole("switch", { name: "Sound" });
		await waitFor(() => expect(sw).not.toBeChecked());
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC12: a read at the same or a higher revision than the early change wins", async () => {
		// Equal revisions mean the same settings, so the read agrees with the
		// event there; at a higher revision the read's own values show.
		for (const [readRevision, readEnabled] of [
			[1, false],
			[2, true],
		] as const) {
			vi.resetAllMocks();
			const core = healthyCore();
			const read = deferred<ReturnType<typeof state>>();
			mocked.getSettings.mockImplementation(() => read.promise);
			const { unmount } = render(<App />);
			await waitFor(() => expect(core.listenerCount()).toBe(1));
			await waitFor(() => expect(mocked.getSettings).toHaveBeenCalled());

			act(() => {
				core.emit(result(state(), { enabled: false }, { revision: 1 }));
			});
			await act(async () => {
				read.resolve(
					state({
						settings: { ...state().settings, enabled: readEnabled },
						revision: readRevision,
					}),
				);
			});
			const sw = await screen.findByRole("switch", { name: "Sound" });
			if (readEnabled) {
				expect(sw).toBeChecked();
			} else {
				expect(sw).not.toBeChecked();
			}
			expect(mocked.setSettings).not.toHaveBeenCalled();
			unmount();
		}
	});

	it("AC12/AC13: an early event's failed save is shown when the read is at the same revision", async () => {
		const core = healthyCore();
		const read = deferred<ReturnType<typeof state>>();
		mocked.getSettings.mockImplementation(() => read.promise);
		render(<App />);
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalled());

		act(() => {
			core.emit(
				result(
					state(),
					{ enabled: false },
					{ revision: 1, saved: false, save_error: "disk is full" },
				),
			);
		});
		await act(async () => {
			read.resolve(
				state({
					settings: { ...state().settings, enabled: false },
					revision: 1,
				}),
			);
		});
		const sw = await screen.findByRole("switch", { name: "Sound" });
		expect(sw).not.toBeChecked();
		expect(await screen.findByText(NOTICE)).toHaveTextContent("disk is full");
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC12: if the subscription is rejected the window still loads", async () => {
		healthyCore();
		mocked.onSettingsChanged.mockRejectedValue("no events");
		const sw = await loaded();
		expect(sw).toBeChecked();
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});
});
