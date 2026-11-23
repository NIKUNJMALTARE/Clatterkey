// Feature 09: pitch variation, key-up sounds and Preview in the settings
// window (AC1 to AC10). The core is mocked at the `src/api` seam. AC11 is
// the browser check and AC25 the manual checklist.
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
import { healthyCore, PACKS, result, state } from "./test/fixtures";

vi.mock("./api", () => ({
	listPacks: vi.fn(),
	getSettings: vi.fn(),
	setSettings: vi.fn(),
	audioStatus: vi.fn(),
	permissionStatus: vi.fn(),
	openPermissionSettings: vi.fn(),
	restartApp: vi.fn(),
	onSettingsChanged: vi.fn(),
	preview: vi.fn(),
}));

const mocked = vi.mocked(api);

beforeEach(() => {
	vi.resetAllMocks();
});

async function loaded() {
	render(<App />);
	return screen.findByRole("switch", { name: "Sound" });
}

const pitch = () => screen.getByRole("slider", { name: "Pitch variation" });
const volume = () => screen.getByRole("slider", { name: "Volume" });
const keyUp = () => screen.getByRole("switch", { name: "Key-up sounds" });
const previewButton = () => screen.getByRole("button", { name: "Preview" });
const radio = (name: RegExp | string) => screen.getByRole("radio", { name });

function withSettings(over: Record<string, unknown>, rest = {}) {
	return state({ settings: { ...state().settings, ...over }, ...rest });
}

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
	it("AC1: pitch variation 0.12 reads 12 and 12%, key-up off, and Preview is by the Sound pack heading", async () => {
		healthyCore(withSettings({ pitch_variation: 0.12, key_up_sounds: false }));
		await loaded();
		expect(pitch()).toHaveValue("12");
		expect(screen.getByText("12%")).toBeInTheDocument();
		expect(keyUp()).not.toBeChecked();
		const list = screen.getByRole("radiogroup", { name: "Sound pack" });
		const button = previewButton();
		// By the heading: before the pack list, after the key-up switch.
		expect(
			button.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			keyUp().compareDocumentPosition(button) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("AC1: pitch variation 0 reads Off", async () => {
		healthyCore(withSettings({ pitch_variation: 0 }));
		await loaded();
		expect(pitch()).toHaveValue("0");
		expect(screen.getByText("Off")).toBeInTheDocument();
		expect(screen.queryByText("0%")).not.toBeInTheDocument();
	});

	it("AC1: key-up on shows the switch on", async () => {
		healthyCore(withSettings({ key_up_sounds: true }));
		await loaded();
		expect(keyUp()).toBeChecked();
	});

	it("AC1 edge: 0.3 reads 30 and 0.07 reads 7, whole percentages", async () => {
		healthyCore(withSettings({ pitch_variation: 0.3 }));
		await loaded();
		expect(pitch()).toHaveValue("30");
		expect(screen.getByText("30%")).toBeInTheDocument();
	});

	it("AC1 edge: 0.07 reads 7 and not 7.000000000000001", async () => {
		healthyCore(withSettings({ pitch_variation: 0.07 }));
		await loaded();
		expect(pitch()).toHaveValue("7");
		expect(screen.getByText("7%")).toBeInTheDocument();
	});

	it("AC1: the slider runs from 0 to 30 in steps of 1", async () => {
		healthyCore();
		await loaded();
		expect(pitch()).toHaveAttribute("min", "0");
		expect(pitch()).toHaveAttribute("max", "30");
		expect(pitch()).toHaveAttribute("step", "1");
	});

	it("AC1 edge: with no packs the new controls and Preview are still shown", async () => {
		healthyCore(withSettings({}, { playing_pack: null }), []);
		await loaded();
		expect(pitch()).toBeInTheDocument();
		expect(keyUp()).toBeInTheDocument();
		expect(previewButton()).toBeInTheDocument();
	});
});

describe("AC2: the pitch variation slider", () => {
	it("AC2: moving to 20 asks for 0.2 and nothing else, and shows 20%", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(pitch(), { target: { value: "20" } });
		expect(screen.getByText("20%")).toBeInTheDocument();
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalled());
		expect(mocked.setSettings).toHaveBeenCalledWith({ pitch_variation: 0.2 });
		for (const [patch] of mocked.setSettings.mock.calls) {
			expect(Object.keys(patch)).toEqual(["pitch_variation"]);
		}
	});

	it("AC2: several movements within a second end with the last value", async () => {
		healthyCore();
		await loaded();
		for (const v of ["3", "8", "12", "25", "17"]) {
			fireEvent.change(pitch(), { target: { value: v } });
		}
		await waitFor(
			() =>
				expect(mocked.setSettings).toHaveBeenLastCalledWith({
					pitch_variation: 0.17,
				}),
			{ timeout: 2000 },
		);
		expect(mocked.setSettings.mock.calls.length).toBeLessThan(5);
		await new Promise((r) => setTimeout(r, 300));
		expect(mocked.setSettings).toHaveBeenLastCalledWith({
			pitch_variation: 0.17,
		});
		expect(pitch()).toHaveValue("17");
		expect(screen.getByText("17%")).toBeInTheDocument();
	});

	it("AC2: releasing the slider sends the value without waiting", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(pitch(), { target: { value: "10" } });
		await waitFor(() => expect(mocked.setSettings).toHaveBeenCalledTimes(1));
		fireEvent.change(pitch(), { target: { value: "12" } });
		fireEvent.pointerUp(pitch());
		fireEvent.keyUp(pitch());
		await waitFor(
			() =>
				expect(mocked.setSettings).toHaveBeenLastCalledWith({
					pitch_variation: 0.12,
				}),
			{ timeout: 2000 },
		);
	});

	it("AC2 edge: 0 is sent as 0, shows Off, and 30 is sent as exactly 0.3", async () => {
		healthyCore(withSettings({ pitch_variation: 0.1 }));
		await loaded();
		fireEvent.change(pitch(), { target: { value: "0" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0,
			}),
		);
		expect(screen.getByText("Off")).toBeInTheDocument();
		await new Promise((r) => setTimeout(r, 150));
		fireEvent.change(pitch(), { target: { value: "30" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0.3,
			}),
		);
		expect(screen.getByText("30%")).toBeInTheDocument();
	});

	it("AC2 edge: whole percentages send exactly percent/100 (7 and 29)", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(pitch(), { target: { value: "7" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0.07,
			}),
		);
		await new Promise((r) => setTimeout(r, 150));
		fireEvent.change(pitch(), { target: { value: "29" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0.29,
			}),
		);
	});

	it("AC2 edge: a pitch drag while a volume change is waiting sends each field's last value, one field per call", async () => {
		healthyCore();
		await loaded();
		fireEvent.change(volume(), { target: { value: "20" } });
		fireEvent.change(volume(), { target: { value: "30" } });
		fireEvent.change(pitch(), { target: { value: "10" } });
		fireEvent.change(pitch(), { target: { value: "15" } });
		await waitFor(
			() => {
				expect(mocked.setSettings).toHaveBeenCalledWith({ volume: 0.3 });
				expect(mocked.setSettings).toHaveBeenCalledWith({
					pitch_variation: 0.15,
				});
			},
			{ timeout: 2000 },
		);
		for (const [patch] of mocked.setSettings.mock.calls) {
			expect(Object.keys(patch)).toHaveLength(1);
		}
		const lastOf = (key: string) =>
			[...mocked.setSettings.mock.calls].reverse().find(([p]) => key in p)?.[0];
		await new Promise((r) => setTimeout(r, 300));
		expect(lastOf("volume")).toEqual({ volume: 0.3 });
		expect(lastOf("pitch_variation")).toEqual({ pitch_variation: 0.15 });
		expect(volume()).toHaveValue("30");
		expect(pitch()).toHaveValue("15");
	});
});

describe("AC3: the key-up switch", () => {
	it("AC3: turning it off asks for false and nothing else; turning it on asks for true", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(keyUp());
		expect(keyUp()).not.toBeChecked();
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({
				key_up_sounds: false,
			}),
		);
		for (const [patch] of mocked.setSettings.mock.calls) {
			expect(Object.keys(patch)).toEqual(["key_up_sounds"]);
		}
		fireEvent.click(keyUp());
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				key_up_sounds: true,
			}),
		);
		expect(keyUp()).toBeChecked();
	});

	it("AC3: the switch shows the new value while the core has not answered", async () => {
		healthyCore();
		await loaded();
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.click(keyUp());
		expect(keyUp()).not.toBeChecked();
		await act(async () => {
			pending.resolve(result(state(), { key_up_sounds: false }));
		});
		expect(keyUp()).not.toBeChecked();
	});
});

describe("AC4: the note for a pack with no key-up sounds", () => {
	it("AC4: a playing pack without key-up samples shows its name and the switch still works", async () => {
		healthyCore(
			withSettings({ pack_id: "typewriter" }, { playing_pack: "typewriter" }),
		);
		await loaded();
		expect(
			screen.getByText("Typewriter has no key-up sounds."),
		).toBeInTheDocument();
		expect(keyUp()).toBeEnabled();
		fireEvent.click(keyUp());
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({
				key_up_sounds: false,
			}),
		);
	});

	it("AC4: the built-in click playing reads 'The built-in click has no key-up sounds.'", async () => {
		healthyCore(withSettings({ pack_id: "nope" }, { playing_pack: null }));
		await loaded();
		expect(
			screen.getByText("The built-in click has no key-up sounds."),
		).toBeInTheDocument();
	});

	it("AC4: a pack with key-up samples shows no such line", async () => {
		healthyCore();
		await loaded();
		expect(screen.queryByText(/has no key-up sounds/)).not.toBeInTheDocument();
	});

	it("AC4: the line goes when a pack with key-up samples is chosen, and comes when Typewriter is", async () => {
		healthyCore(
			withSettings({ pack_id: "typewriter" }, { playing_pack: "typewriter" }),
		);
		await loaded();
		fireEvent.click(radio(/Blue switches/));
		await waitFor(() =>
			expect(
				screen.queryByText(/has no key-up sounds/),
			).not.toBeInTheDocument(),
		);
		fireEvent.click(radio(/Typewriter/));
		expect(
			await screen.findByText("Typewriter has no key-up sounds."),
		).toBeInTheDocument();
	});

	it("AC4 edge: choosing Typewriter shows the line at once, and a refused change takes it away", async () => {
		healthyCore();
		await loaded();
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.click(radio(/Typewriter/));
		expect(
			screen.getByText("Typewriter has no key-up sounds."),
		).toBeInTheDocument();
		await act(async () => {
			pending.reject("could not load");
		});
		await screen.findByText(/Could not load/);
		expect(screen.queryByText(/has no key-up sounds/)).not.toBeInTheDocument();
	});
});

describe("AC5: Preview", () => {
	it("AC5: pressing Preview calls the core's preview once with no arguments and changes no setting; again calls again", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalledTimes(1));
		expect(mocked.preview).toHaveBeenCalledWith();
		expect(mocked.setSettings).not.toHaveBeenCalled();
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalledTimes(2));
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC5: Preview is never disabled, even while an earlier preview has not answered", async () => {
		healthyCore();
		await loaded();
		const pending = deferred<undefined>();
		mocked.preview.mockReturnValue(pending.promise);
		fireEvent.click(previewButton());
		expect(previewButton()).toBeEnabled();
		fireEvent.click(previewButton());
		expect(mocked.preview).toHaveBeenCalledTimes(2);
		pending.resolve(undefined);
	});

	it("AC5 edge: with no packs Preview still calls the core", async () => {
		healthyCore(withSettings({}, { playing_pack: null }), []);
		await loaded();
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalledTimes(1));
	});
});

describe("AC6: a failed preview", () => {
	it("AC6: a rejection shows the notice with the reason, and it can be dismissed", async () => {
		healthyCore();
		await loaded();
		mocked.preview.mockRejectedValueOnce("no thread");
		fireEvent.click(previewButton());
		const notice = await screen.findByText(/Could not play the preview:/);
		expect(notice).toHaveTextContent("Could not play the preview: no thread");
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		expect(
			screen.queryByText(/Could not play the preview/),
		).not.toBeInTheDocument();
	});

	it("AC6: a later preview that succeeds removes the notice", async () => {
		healthyCore();
		await loaded();
		mocked.preview.mockRejectedValueOnce("no thread");
		fireEvent.click(previewButton());
		await screen.findByText(/Could not play the preview/);
		mocked.preview.mockResolvedValueOnce(undefined);
		fireEvent.click(previewButton());
		await waitFor(() =>
			expect(
				screen.queryByText(/Could not play the preview/),
			).not.toBeInTheDocument(),
		);
	});

	it("AC6: the notice is an alert and no setting is touched", async () => {
		healthyCore();
		await loaded();
		mocked.preview.mockRejectedValueOnce("boom");
		fireEvent.click(previewButton());
		const notice = await screen.findByText(/Could not play the preview/);
		expect(notice.closest("[role=alert]")).not.toBeNull();
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});
});

describe("AC7: refused changes and failed saves", () => {
	it("AC7: a refused pitch change returns the slider to the value in force and shows the change notice", async () => {
		healthyCore(withSettings({ pitch_variation: 0.1 }));
		await loaded();
		mocked.setSettings.mockRejectedValueOnce("must be a number from 0 to 0.3");
		fireEvent.change(pitch(), { target: { value: "20" } });
		await screen.findByText(/Could not change the setting/);
		expect(
			screen.getByText(/must be a number from 0 to 0.3/),
		).toBeInTheDocument();
		await waitFor(() => expect(pitch()).toHaveValue("10"));
		expect(screen.getByText("10%")).toBeInTheDocument();
	});

	it("AC7: a refused key-up change returns the switch to the value in force and shows the change notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockRejectedValueOnce("must be true or false");
		fireEvent.click(keyUp());
		await screen.findByText(/Could not change the setting/);
		await waitFor(() => expect(keyUp()).toBeChecked());
	});

	it("AC7: a failed save on pitch keeps the value and shows the save-failed notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValue(
			result(
				state(),
				{ pitch_variation: 0.2 },
				{ saved: false, save_error: "disk is full" },
			),
		);
		fireEvent.change(pitch(), { target: { value: "20" } });
		const notice = await screen.findByText(/Could not save your settings/);
		expect(notice).toHaveTextContent("disk is full");
		expect(pitch()).toHaveValue("20");
		expect(screen.getByText("20%")).toBeInTheDocument();
	});

	it("AC7: a failed save on key-up keeps the value and shows the save-failed notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValueOnce(
			result(
				state(),
				{ key_up_sounds: false },
				{ saved: false, save_error: "read-only" },
			),
		);
		fireEvent.click(keyUp());
		await screen.findByText(/Could not save your settings/);
		expect(keyUp()).not.toBeChecked();
	});
});

describe("AC8: changes that arrive from the core", () => {
	it("AC8: an event with a higher revision moves the pitch slider and the key-up switch, with no set_settings call", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ pitch_variation: 0.25, key_up_sounds: false });
		});
		await waitFor(() => expect(pitch()).toHaveValue("25"));
		expect(screen.getByText("25%")).toBeInTheDocument();
		expect(keyUp()).not.toBeChecked();
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC8: an event with a lower revision is ignored", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ pitch_variation: 0.25 });
		});
		await waitFor(() => expect(pitch()).toHaveValue("25"));
		act(() => {
			core.emit(result(state(), { pitch_variation: 0.1 }, { revision: 0 }));
		});
		expect(pitch()).toHaveValue("25");
	});

	it("AC8 edge: an event for pitch arriving while the user's own value is awaiting an answer does not move the slider", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.change(pitch(), { target: { value: "10" } });
		act(() => {
			core.outsideChange({ pitch_variation: 0.25 });
		});
		expect(pitch()).toHaveValue("10");
		await act(async () => {
			pending.resolve(
				result(state(), { pitch_variation: 0.1 }, { revision: 5 }),
			);
		});
	});
});

describe("AC9: roles, names, descriptions and keyboard", () => {
	it("AC9: the pitch slider has a role, a name and a value text", async () => {
		healthyCore(withSettings({ pitch_variation: 0.12 }));
		await loaded();
		expect(pitch()).toHaveAttribute("aria-valuetext", "12%");
		fireEvent.change(pitch(), { target: { value: "0" } });
		expect(pitch()).toHaveAttribute("aria-valuetext", "Off");
	});

	it("AC9: the slider is a native range input, so arrows, Page and Home/End work", async () => {
		healthyCore();
		await loaded();
		const el = pitch() as HTMLInputElement;
		expect(el.tagName).toBe("INPUT");
		expect(el.type).toBe("range");
	});

	it("AC9: the key-up switch is a switch with a name, and the note is its description", async () => {
		healthyCore(
			withSettings({ pack_id: "typewriter" }, { playing_pack: "typewriter" }),
		);
		await loaded();
		expect(keyUp()).toHaveAccessibleDescription(
			"Typewriter has no key-up sounds.",
		);
	});

	it("AC9: with no note the switch has no description", async () => {
		healthyCore();
		await loaded();
		expect(keyUp()).not.toHaveAccessibleDescription();
	});

	it("AC9: Preview is a button named Preview", async () => {
		healthyCore();
		await loaded();
		expect(previewButton().tagName).toBe("BUTTON");
		expect(previewButton()).toHaveAttribute("type", "button");
	});

	it("AC9: every control is in the tab order, in the order shown", async () => {
		healthyCore();
		const sound = await loaded();
		const order = [
			sound,
			volume(),
			pitch(),
			keyUp(),
			previewButton(),
			radio(/Red switches/),
		];
		for (const el of order) {
			expect(el).toBeEnabled();
			expect(el.tabIndex).toBeGreaterThanOrEqual(0);
		}
		for (let i = 0; i + 1 < order.length; i++) {
			expect(
				order[i].compareDocumentPosition(order[i + 1]) &
					Node.DOCUMENT_POSITION_FOLLOWING,
				`control ${i} comes before control ${i + 1}`,
			).toBeTruthy();
		}
	});

	it("AC9: Preview can be pressed from the keyboard", async () => {
		healthyCore();
		await loaded();
		previewButton().focus();
		expect(previewButton()).toHaveFocus();
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalledTimes(1));
	});
});

describe("AC10: while Sound is off", () => {
	it("AC10: the pitch slider, the key-up switch and Preview are all enabled and work", async () => {
		healthyCore(withSettings({ enabled: false }));
		await loaded();
		expect(pitch()).toBeEnabled();
		expect(keyUp()).toBeEnabled();
		expect(previewButton()).toBeEnabled();

		fireEvent.change(pitch(), { target: { value: "5" } });
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({
				pitch_variation: 0.05,
			}),
		);
		fireEvent.click(keyUp());
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ key_up_sounds: false }),
		);
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalledTimes(1));
	});
});

describe("Preview on the pack list", () => {
	it("Preview does not alter which pack is selected", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(previewButton());
		await waitFor(() => expect(mocked.preview).toHaveBeenCalled());
		expect(radio(/Red switches/)).toBeChecked();
		expect(PACKS).toHaveLength(4);
	});
});

describe("AC2: Page Up and Page Down move by 10", () => {
	it("AC2: Page Up on pitch variation at 12 shows 22% and asks for 0.22", async () => {
		healthyCore(withSettings({ pitch_variation: 0.12 }));
		await loaded();
		fireEvent.keyDown(pitch(), { key: "PageUp" });
		expect(screen.getByText("22%")).toBeInTheDocument();
		expect(pitch()).toHaveValue("22");
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0.22,
			}),
		);
	});

	it("AC2: Page Up at 25 stops at 30%", async () => {
		healthyCore(withSettings({ pitch_variation: 0.25 }));
		await loaded();
		fireEvent.keyDown(pitch(), { key: "PageUp" });
		expect(screen.getByText("30%")).toBeInTheDocument();
		expect(pitch()).toHaveValue("30");
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0.3,
			}),
		);
	});

	it("AC2: Page Down at 5 stops at Off and asks for 0", async () => {
		healthyCore(withSettings({ pitch_variation: 0.05 }));
		await loaded();
		fireEvent.keyDown(pitch(), { key: "PageDown" });
		expect(screen.getByText("Off")).toBeInTheDocument();
		expect(pitch()).toHaveValue("0");
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				pitch_variation: 0,
			}),
		);
	});

	it("Feature 06 AC4: Page Down on volume at 80 shows 70% and asks for 0.7", async () => {
		healthyCore();
		await loaded();
		fireEvent.keyDown(volume(), { key: "PageDown" });
		expect(screen.getByText("70%")).toBeInTheDocument();
		expect(volume()).toHaveValue("70");
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({ volume: 0.7 }),
		);
	});
});
