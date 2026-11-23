import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as api from "./api";
import type { SetResult } from "./api/types";
import { PACKS, healthyCore, result, state } from "./test/fixtures";

// The core is mocked at the `src/api` seam.
vi.mock("./api", () => ({
	listPacks: vi.fn(),
	getSettings: vi.fn(),
	setSettings: vi.fn(),
	audioStatus: vi.fn(),
}));

const mocked = vi.mocked(api);

beforeEach(() => {
	vi.resetAllMocks();
});

async function loaded() {
	render(<App />);
	return screen.findByRole("switch", { name: "Sound" });
}

const slider = () => screen.getByRole("slider", { name: "Volume" });
const radio = (name: RegExp | string) => screen.getByRole("radio", { name });
const rows = () => screen.getAllByRole("radio");

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

describe("AC1: the window loads with the saved values", () => {
	it("AC1: shows the switch on, 80% on the slider and four packs in order with Red selected", async () => {
		healthyCore();
		const sw = await loaded();
		expect(sw).toBeChecked();
		expect(slider()).toHaveValue("80");
		expect(screen.getByText("80%")).toBeInTheDocument();

		const names = rows().map((r) => r.closest("label")?.textContent ?? "");
		expect(names).toHaveLength(4);
		const order = ["Blue", "Brown", "Red", "Typewriter"];
		order.forEach((n, i) => {
			expect(names[i]).toContain(n);
		});
		expect(
			screen.getAllByText("Mechanical Key Sounds contributors · CC0-1.0"),
		).toHaveLength(4);
		expect(radio(/Red switches/)).toBeChecked();
		expect(radio(/Blue switches/)).not.toBeChecked();
		expect(radio(/Brown switches/)).not.toBeChecked();
		expect(radio(/Typewriter/)).not.toBeChecked();
	});

	it("AC1 edge: a pack with no author shows the licence alone", async () => {
		healthyCore(state(), [
			{ id: "blue", name: "Blue switches", author: null, license: "MIT" },
			PACKS[2],
		]);
		await loaded();
		expect(screen.getByText("MIT")).toBeInTheDocument();
		expect(screen.queryByText(/·\s*MIT|MIT\s*·/)).not.toBeInTheDocument();
	});

	it("AC1 edge: a volume of 0.35 reads 35", async () => {
		healthyCore(state({ settings: { ...state().settings, volume: 0.35 } }));
		await loaded();
		expect(slider()).toHaveValue("35");
		expect(screen.getByText("35%")).toBeInTheDocument();
	});

	it("AC1 edge: a switch saved as off shows off", async () => {
		healthyCore(state({ settings: { ...state().settings, enabled: false } }));
		expect(await loaded()).not.toBeChecked();
	});

	it("AC1: the window has the app name as a heading", async () => {
		healthyCore();
		await loaded();
		expect(
			screen.getByRole("heading", { name: "Mechanical Key Sounds" }),
		).toBeInTheDocument();
	});
});

describe("AC2: loading and the first read failing", () => {
	it("AC2: shows no control, only the title, until settings and packs have arrived", async () => {
		const s = deferred<ReturnType<typeof state>>();
		const p = deferred<typeof PACKS>();
		mocked.getSettings.mockReturnValue(s.promise);
		mocked.listPacks.mockReturnValue(p.promise);
		mocked.audioStatus.mockResolvedValue("playing");
		render(<App />);
		expect(
			screen.getByRole("heading", { name: "Mechanical Key Sounds" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("switch")).not.toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
		expect(screen.queryByRole("radio")).not.toBeInTheDocument();

		// Settings alone are not enough.
		s.resolve(state());
		await Promise.resolve();
		expect(screen.queryByRole("switch")).not.toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();

		p.resolve(PACKS);
		expect(await screen.findByRole("switch", { name: "Sound" })).toBeChecked();
		expect(slider()).toHaveValue("80");
	});

	it("AC2: a failed first read shows the message and Retry, and no controls", async () => {
		mocked.getSettings.mockRejectedValue("core not ready");
		mocked.listPacks.mockResolvedValue(PACKS);
		mocked.audioStatus.mockResolvedValue("playing");
		render(<App />);
		expect(
			await screen.findByText("Settings could not be loaded."),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
		expect(screen.queryByRole("switch")).not.toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
		expect(screen.queryByRole("radio")).not.toBeInTheDocument();
	});

	it("AC2: a failed pack list also shows the error", async () => {
		mocked.getSettings.mockResolvedValue(state());
		mocked.listPacks.mockRejectedValue("no");
		render(<App />);
		expect(
			await screen.findByText("Settings could not be loaded."),
		).toBeInTheDocument();
	});

	it("AC2: Retry reads again and shows the controls when it works", async () => {
		mocked.getSettings
			.mockRejectedValueOnce("core not ready")
			.mockResolvedValue(state());
		mocked.listPacks.mockResolvedValue(PACKS);
		mocked.audioStatus.mockResolvedValue("playing");
		render(<App />);
		fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalledTimes(2));
		expect(await screen.findByRole("switch", { name: "Sound" })).toBeChecked();
		expect(
			screen.queryByText("Settings could not be loaded."),
		).not.toBeInTheDocument();
	});

	it("AC2 edge: Retry that fails again stays on the error", async () => {
		mocked.getSettings.mockRejectedValue("still not ready");
		mocked.listPacks.mockResolvedValue(PACKS);
		render(<App />);
		fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
		await waitFor(() => expect(mocked.getSettings).toHaveBeenCalledTimes(2));
		expect(
			await screen.findByText("Settings could not be loaded."),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
	});
});

describe("AC3: the Sound switch", () => {
	it("AC3: turning it off asks for enabled false and nothing else, and it shows off", async () => {
		healthyCore();
		const sw = await loaded();
		fireEvent.click(sw);
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		expect(mocked.setSettings).toHaveBeenCalledWith({ enabled: false });
		expect(screen.getByRole("switch", { name: "Sound" })).not.toBeChecked();
	});

	it("AC3: turning it on again asks for true", async () => {
		healthyCore();
		const sw = await loaded();
		fireEvent.click(sw);
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		fireEvent.click(screen.getByRole("switch", { name: "Sound" }));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(2));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({ enabled: true });
		expect(screen.getByRole("switch", { name: "Sound" })).toBeChecked();
	});

	it("AC3 edge: the switch shows the new value at once, before the core answers", async () => {
		healthyCore();
		const sw = await loaded();
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.click(sw);
		expect(screen.getByRole("switch", { name: "Sound" })).not.toBeChecked();
		pending.resolve(result(state(), { enabled: false }));
	});

	it("AC3 edge: while off, volume and pack list stay usable", async () => {
		healthyCore(state({ settings: { ...state().settings, enabled: false } }));
		await loaded();
		expect(slider()).toBeEnabled();
		for (const r of rows()) expect(r).toBeEnabled();
		fireEvent.click(radio(/Blue switches/));
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ pack_id: "blue" }),
		);
	});
});

describe("AC4: the volume slider", () => {
	it("AC4: moving to 35 asks for 0.35 and nothing else, and shows 35%", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(slider(), { target: { value: "35" } });
		expect(screen.getByText("35%")).toBeInTheDocument();
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalled());
		expect(mocked.setSettings).toHaveBeenCalledWith({ volume: 0.35 });
		for (const [patch] of mocked.setSettings.mock.calls) {
			expect(Object.keys(patch)).toEqual(["volume"]);
		}
	});

	it("AC4: several movements within a second end with the last value", async () => {
		healthyCore();
		await loaded();
		for (const v of ["10", "20", "30", "40", "55", "70", "35"]) {
			fireEvent.change(slider(), { target: { value: v } });
		}
		await waitFor(
			() =>
				expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.35 }),
			{ timeout: 2000 },
		);
		// Throttled: not one call per movement.
		expect(mocked.setSettings.mock.calls.length).toBeLessThan(7);
		await new Promise((r) => setTimeout(r, 300));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.35 });
		expect(screen.getByText("35%")).toBeInTheDocument();
		expect(slider()).toHaveValue("35");
	});

	it("AC4: releasing the slider sends the value without waiting", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(slider(), { target: { value: "10" } });
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		fireEvent.change(slider(), { target: { value: "12" } });
		fireEvent.pointerUp(slider());
		fireEvent.mouseUp(slider());
		fireEvent.keyUp(slider());
		await waitFor(
			() =>
				expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.12 }),
			{ timeout: 2000 },
		);
	});

	it("AC4 edge: 0 and 100 are sent as 0 and 1", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(slider(), { target: { value: "0" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0 }),
		);
		expect(screen.getByText("0%")).toBeInTheDocument();
		await new Promise((r) => setTimeout(r, 150));
		fireEvent.change(slider(), { target: { value: "100" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 1 }),
		);
		expect(screen.getByText("100%")).toBeInTheDocument();
	});

	it("AC4: the slider runs from 0 to 100 in steps of 1", async () => {
		healthyCore();
		await loaded();
		expect(slider()).toHaveAttribute("min", "0");
		expect(slider()).toHaveAttribute("max", "100");
		expect(slider()).toHaveAttribute("step", "1");
	});
});

describe("AC4: the slider while an answer is outstanding", () => {
	it("AC4: a newer value waiting for the throttle is not undone by the older answer", async () => {
		healthyCore();
		await loaded();
		const first = deferred<SetResult>();
		mocked.setSettings
			.mockReturnValueOnce(first.promise)
			.mockImplementation(async (patch) => result(state(), patch));
		fireEvent.change(slider(), { target: { value: "10" } });
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.1 });

		// A newer value, waiting in the throttle.
		fireEvent.change(slider(), { target: { value: "20" } });
		await act(async () => {
			first.resolve(result(state(), { volume: 0.1 }));
		});
		expect(slider()).toHaveValue("20");
		expect(screen.getByText("20%")).toBeInTheDocument();
		expect(screen.queryByText("10%")).not.toBeInTheDocument();

		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(2));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.2 });
		await new Promise((r) => setTimeout(r, 50));
		expect(slider()).toHaveValue("20");
		expect(screen.getByText("20%")).toBeInTheDocument();
	});
});

describe("AC6: a stale pack failure", () => {
	it("AC6: a rejection of an earlier choice does not leave a notice once a later choice has loaded", async () => {
		healthyCore();
		await loaded();
		const a = deferred<SetResult>();
		mocked.setSettings
			.mockReturnValueOnce(a.promise)
			.mockImplementation(async (patch) => result(state(), patch));
		fireEvent.click(radio(/Blue switches/));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		fireEvent.click(radio(/Typewriter/));
		a.reject("blue is broken");
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(2));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({
			pack_id: "typewriter",
		});
		await waitFor(() =>
			expect(screen.queryByText(/Could not load/)).not.toBeInTheDocument(),
		);
		await new Promise((r) => setTimeout(r, 50));
		expect(screen.queryByText(/Could not load/)).not.toBeInTheDocument();
		expect(radio(/Typewriter/)).toBeChecked();
		expect(radio(/Blue switches/)).not.toBeChecked();
	});
});

describe("AC5: choosing a pack", () => {
	it("AC5: choosing Blue switches asks for pack_id blue and nothing else, and selects the row", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(radio(/Blue switches/));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		expect(mocked.setSettings).toHaveBeenCalledWith({ pack_id: "blue" });
		expect(radio(/Blue switches/)).toBeChecked();
		expect(radio(/Red switches/)).not.toBeChecked();
	});

	it("AC5 edge: the row is selected at once, before the core answers", async () => {
		healthyCore();
		await loaded();
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.click(radio(/Typewriter/));
		expect(radio(/Typewriter/)).toBeChecked();
		expect(radio(/Red switches/)).not.toBeChecked();
		pending.resolve(result(state(), { pack_id: "typewriter" }));
	});

	it("AC5 edge: a late answer does not undo a newer choice", async () => {
		healthyCore();
		await loaded();
		const first = deferred<SetResult>();
		mocked.setSettings
			.mockReturnValueOnce(first.promise)
			.mockImplementation(async (patch) => result(state(), patch));
		fireEvent.click(radio(/Blue switches/));
		fireEvent.click(radio(/Typewriter/));
		first.resolve(result(state(), { pack_id: "blue" }));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(2));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({
			pack_id: "typewriter",
		});
		await new Promise((r) => setTimeout(r, 50));
		expect(radio(/Typewriter/)).toBeChecked();
		expect(radio(/Blue switches/)).not.toBeChecked();
	});
});

describe("AC6: a pack that cannot be loaded", () => {
	it("AC6: the selection returns to the playing pack and a notice names the pack and the reason", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockRejectedValue(
			'pack "blue": file "down/a.wav" could not be decoded',
		);
		fireEvent.click(radio(/Blue switches/));
		const notice = await screen.findByText(/Could not load/);
		expect(notice).toHaveTextContent("Blue switches");
		expect(notice).toHaveTextContent("could not be decoded");
		expect(radio(/Red switches/)).toBeChecked();
		expect(radio(/Blue switches/)).not.toBeChecked();
	});

	it("AC6: choosing another pack that loads removes the notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockRejectedValueOnce("broken");
		fireEvent.click(radio(/Blue switches/));
		await screen.findByText(/Could not load/);
		mocked.setSettings.mockImplementation(async (patch) =>
			result(state(), patch),
		);
		fireEvent.click(radio(/Brown switches/));
		await waitFor(() =>
			expect(screen.queryByText(/Could not load/)).not.toBeInTheDocument(),
		);
		expect(radio(/Brown switches/)).toBeChecked();
	});

	it("AC6 edge: a rejected pack change after an earlier good one returns to the earlier one", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(radio(/Brown switches/));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		mocked.setSettings.mockRejectedValueOnce("broken");
		fireEvent.click(radio(/Typewriter/));
		await screen.findByText(/Could not load/);
		expect(radio(/Brown switches/)).toBeChecked();
	});

	it("AC6 edge: with no pack playing a rejected change leaves none selected", async () => {
		healthyCore(state({ playing_pack: null }));
		await loaded();
		mocked.setSettings.mockRejectedValueOnce("broken");
		fireEvent.click(radio(/Blue switches/));
		await screen.findByText(/Could not load/);
		for (const r of rows()) expect(r).not.toBeChecked();
	});
});

describe("AC7: a save that failed", () => {
	it("AC7: the control keeps the new value and the notice shows; the next good save removes it", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValueOnce(
			result(
				state(),
				{ enabled: false },
				{ saved: false, save_error: "disk is full" },
			),
		);
		fireEvent.click(screen.getByRole("switch", { name: "Sound" }));
		const notice = await screen.findByText(/Could not save your settings/);
		expect(notice).toHaveTextContent("disk is full");
		expect(
			screen.getByText(/Changes will last until the app quits/),
		).toBeInTheDocument();
		expect(screen.getByRole("switch", { name: "Sound" })).not.toBeChecked();

		mocked.setSettings.mockResolvedValueOnce(
			result(state(), { enabled: true }),
		);
		fireEvent.click(screen.getByRole("switch", { name: "Sound" }));
		await waitFor(() =>
			expect(
				screen.queryByText(/Could not save your settings/),
			).not.toBeInTheDocument(),
		);
		expect(screen.getByRole("switch", { name: "Sound" })).toBeChecked();
	});

	it("AC7 edge: a failed save on a pack change keeps the pack selected", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValueOnce(
			result(
				state(),
				{ pack_id: "blue" },
				{ saved: false, save_error: "read-only" },
			),
		);
		fireEvent.click(radio(/Blue switches/));
		await screen.findByText(/Could not save your settings/);
		expect(radio(/Blue switches/)).toBeChecked();
	});

	it("AC7 edge: a failed save on the volume keeps the slider value", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValue(
			result(
				state(),
				{ volume: 0.2 },
				{ saved: false, save_error: "no space" },
			),
		);
		fireEvent.change(slider(), { target: { value: "20" } });
		await screen.findByText(/Could not save your settings/);
		expect(slider()).toHaveValue("20");
		expect(screen.getByText("20%")).toBeInTheDocument();
	});
});

describe("AC8: start-up problems", () => {
	it("AC8: an unreadable settings file shows its notice, which can be dismissed", async () => {
		healthyCore(
			state({
				startup: {
					settings_unreadable: true,
					replaced_fields: [],
					unusable_pack: null,
				},
			}),
		);
		await loaded();
		expect(
			screen.getByText(
				/Your settings file could not be read, so the defaults are in use/,
			),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		expect(
			screen.queryByText(/Your settings file could not be read/),
		).not.toBeInTheDocument();
	});

	it("AC8: replaced fields are named, and the notice can be dismissed", async () => {
		healthyCore(
			state({
				startup: {
					settings_unreadable: false,
					replaced_fields: ["volume", "key_up_sounds"],
					unusable_pack: null,
				},
			}),
		);
		await loaded();
		expect(
			screen.getByText(
				/Some settings were invalid and were reset: volume, key_up_sounds/,
			),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		expect(
			screen.queryByText(/Some settings were invalid/),
		).not.toBeInTheDocument();
	});

	it("AC8: an unusable saved pack names the id and the pack playing instead, and can be dismissed", async () => {
		healthyCore(
			state({
				settings: { ...state().settings, pack_id: "nope" },
				playing_pack: "brown",
				startup: {
					settings_unreadable: false,
					replaced_fields: [],
					unusable_pack: "nope",
				},
			}),
		);
		await loaded();
		const notice = screen.getByText(/saved sound pack/i);
		expect(notice).toHaveTextContent('"nope"');
		expect(notice).toHaveTextContent("Brown switches");
		expect(radio(/Brown switches/)).toBeChecked();
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		expect(screen.queryByText(/saved sound pack/i)).not.toBeInTheDocument();
	});

	it("AC8: choosing a pack removes the unusable-pack notice", async () => {
		healthyCore(
			state({
				settings: { ...state().settings, pack_id: "nope" },
				playing_pack: "brown",
				startup: {
					settings_unreadable: false,
					replaced_fields: [],
					unusable_pack: "nope",
				},
			}),
		);
		await loaded();
		fireEvent.click(radio(/Red switches/));
		await waitFor(() =>
			expect(screen.queryByText(/saved sound pack/i)).not.toBeInTheDocument(),
		);
	});

	it("AC8: the built-in click playing means no row is selected, and the notice says so", async () => {
		healthyCore(
			state({
				settings: { ...state().settings, pack_id: "nope" },
				playing_pack: null,
				startup: {
					settings_unreadable: false,
					replaced_fields: [],
					unusable_pack: "nope",
				},
			}),
		);
		await loaded();
		for (const r of rows()) expect(r).not.toBeChecked();
		expect(screen.getByText(/saved sound pack/i)).toHaveTextContent(
			"the built-in click",
		);
	});

	it("AC8 edge: with no problems there is no notice and no Dismiss button", async () => {
		healthyCore();
		await loaded();
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /dismiss/i }),
		).not.toBeInTheDocument();
	});
});

describe("AC9: the audio output notices", () => {
	it("AC9: no device shows the notice, and it goes away without a reload when the output opens", async () => {
		const core = healthyCore(state({ audio: "no_device" }));
		await loaded();
		expect(
			screen.getByText(
				"No audio output device found. Sounds will play when one is connected.",
			),
		).toBeInTheDocument();
		core.setAudio("playing");
		await waitFor(
			() =>
				expect(
					screen.queryByText(/No audio output device found/),
				).not.toBeInTheDocument(),
			{ timeout: 3500 },
		);
		expect(mocked.getSettings).toHaveBeenCalledTimes(1);
		expect(mocked.audioStatus).toHaveBeenCalled();
	}, 8000);

	it("AC9: a device that cannot be opened shows its notice and it goes away when it opens", async () => {
		const core = healthyCore(state({ audio: "failed" }));
		await loaded();
		expect(
			screen.getByText("The audio output could not be opened. Retrying…"),
		).toBeInTheDocument();
		core.setAudio("playing");
		await waitFor(
			() =>
				expect(
					screen.queryByText(/The audio output could not be opened/),
				).not.toBeInTheDocument(),
			{ timeout: 3500 },
		);
		expect(mocked.getSettings).toHaveBeenCalledTimes(1);
	}, 8000);

	it("AC9: a notice appears while the window is open when the device goes away", async () => {
		const core = healthyCore();
		await loaded();
		expect(
			screen.queryByText(/No audio output device found/),
		).not.toBeInTheDocument();
		core.setAudio("no_device");
		expect(
			await screen.findByText(/No audio output device found/, undefined, {
				timeout: 3500,
			}),
		).toBeInTheDocument();
	}, 8000);

	it("AC9 edge: the audio notice cannot be dismissed", async () => {
		healthyCore(state({ audio: "no_device" }));
		await loaded();
		expect(
			screen.queryByRole("button", { name: /dismiss/i }),
		).not.toBeInTheDocument();
	});

	it("AC9 edge: the polling stops when the window is closed", async () => {
		healthyCore();
		await loaded();
		cleanup();
		const calls = mocked.audioStatus.mock.calls.length;
		await new Promise((r) => setTimeout(r, 1300));
		expect(mocked.audioStatus.mock.calls.length).toBe(calls);
	});
});

describe("AC10: no packs", () => {
	it("AC10: the message replaces the list, and the switch and slider still work", async () => {
		healthyCore(state({ playing_pack: null }), []);
		const sw = await loaded();
		expect(
			screen.getByText("No sound packs were found. Reinstall the app."),
		).toBeInTheDocument();
		expect(screen.queryByRole("radio")).not.toBeInTheDocument();

		fireEvent.click(sw);
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ enabled: false }),
		);
		expect(screen.getByRole("switch", { name: "Sound" })).not.toBeChecked();

		fireEvent.change(slider(), { target: { value: "50" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ volume: 0.5 }),
		);
		expect(screen.getByText("50%")).toBeInTheDocument();
	});

	it("AC10 edge: with packs the message is not shown", async () => {
		healthyCore();
		await loaded();
		expect(
			screen.queryByText(/No sound packs were found/),
		).not.toBeInTheDocument();
	});
});

describe("notices are ordered and scoped", () => {
	it("worst first: an audio notice comes before a start-up notice", async () => {
		healthyCore(
			state({
				audio: "no_device",
				startup: {
					settings_unreadable: false,
					replaced_fields: ["volume"],
					unusable_pack: null,
				},
			}),
		);
		await loaded();
		const audio = screen.getByText(/No audio output device found/);
		const reset = screen.getByText(/Some settings were invalid/);
		expect(
			audio.compareDocumentPosition(reset) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("a dismissed notice stays dismissed while the window is open", async () => {
		healthyCore(
			state({
				startup: {
					settings_unreadable: false,
					replaced_fields: ["volume"],
					unusable_pack: null,
				},
			}),
		);
		await loaded();
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		fireEvent.click(screen.getByRole("switch", { name: "Sound" }));
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalled());
		await new Promise((r) => setTimeout(r, 50));
		expect(
			screen.queryByText(/Some settings were invalid/),
		).not.toBeInTheDocument();
	});
});
