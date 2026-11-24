// Feature 11: the Launch at login switch in the settings window (AC1 to
// AC7). The core is mocked at the `src/api` seam. AC8 is the browser check
// and AC22 the manual checklist.
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
	preview: vi.fn(),
	importPack: vi.fn(),
	removePack: vi.fn(),
}));

const mocked = vi.mocked(api);

beforeEach(() => {
	vi.resetAllMocks();
});

const NOTE = "Starts hidden in the tray when you log in.";

async function loaded() {
	render(<App />);
	return screen.findByRole("switch", { name: "Sound" });
}

const launch = () => screen.getByRole("switch", { name: "Launch at login" });

function withSettings(over: Record<string, unknown>, rest = {}) {
	return state({ settings: { ...state().settings, ...over }, ...rest });
}

function deferred<T>() {
	let resolve!: (v: T) => void;
	const promise = new Promise<T>((res) => {
		resolve = res;
	});
	return { promise, resolve };
}

describe("AC1: the switch shows the saved value, below the pack list", () => {
	it("AC1: false shows it off, with the line below it", async () => {
		healthyCore(withSettings({ launch_at_login: false }));
		await loaded();
		expect(launch()).not.toBeChecked();
		expect(screen.getByText(NOTE)).toBeInTheDocument();
	});

	it("AC1: true shows it on", async () => {
		healthyCore(withSettings({ launch_at_login: true }));
		await loaded();
		expect(launch()).toBeChecked();
	});

	it("AC1: it comes after the pack list, and the line after the switch", async () => {
		healthyCore();
		await loaded();
		const list = screen.getByRole("radiogroup", { name: "Sound pack" });
		const follows = (a: Node, b: Node) =>
			Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
		expect(follows(list, launch())).toBe(true);
		expect(follows(launch(), screen.getByText(NOTE))).toBe(true);
	});

	it("AC1 edge: with no packs the switch is still shown", async () => {
		healthyCore(withSettings({}, { playing_pack: null }), []);
		await loaded();
		expect(launch()).toBeInTheDocument();
	});
});

describe("AC2: turning it on and off", () => {
	it("AC2: on asks for true and nothing else, off asks for false", async () => {
		healthyCore();
		await loaded();
		fireEvent.click(launch());
		expect(launch()).toBeChecked();
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({
				launch_at_login: true,
			}),
		);
		expect(launch()).toBeChecked();
		fireEvent.click(launch());
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenLastCalledWith({
				launch_at_login: false,
			}),
		);
		expect(launch()).not.toBeChecked();
		for (const [patch] of mocked.setSettings.mock.calls) {
			expect(Object.keys(patch)).toEqual(["launch_at_login"]);
		}
	});

	it("AC2: the switch shows the new value while the core has not answered", async () => {
		healthyCore();
		await loaded();
		const pending = deferred<SetResult>();
		mocked.setSettings.mockReturnValue(pending.promise);
		fireEvent.click(launch());
		expect(launch()).toBeChecked();
		await act(async () => {
			pending.resolve(result(state(), { launch_at_login: true }));
		});
		expect(launch()).toBeChecked();
	});
});

describe("AC3: a refused change and a failed save", () => {
	it("AC3: a refusal returns the switch and shows the reason in the change notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockRejectedValueOnce(
			"settings change: launch at login could not be turned on: access denied",
		);
		fireEvent.click(launch());
		const notice = await screen.findByText(/Could not change the setting: /);
		expect(notice).toHaveTextContent(
			"Could not change the setting: settings change: launch at login could not be turned on: access denied",
		);
		await waitFor(() => expect(launch()).not.toBeChecked());
	});

	it("AC3: a refused turn-off returns the switch to on", async () => {
		healthyCore(withSettings({ launch_at_login: true }));
		await loaded();
		mocked.setSettings.mockRejectedValueOnce(
			"settings change: launch at login could not be turned off: locked",
		);
		fireEvent.click(launch());
		await screen.findByText(/Could not change the setting/);
		await waitFor(() => expect(launch()).toBeChecked());
	});

	it("AC3: a failed save keeps the new value and shows the save-failed notice", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockResolvedValueOnce(
			result(
				state(),
				{ launch_at_login: true },
				{ saved: false, save_error: "disk is full" },
			),
		);
		fireEvent.click(launch());
		const notice = await screen.findByText(/Could not save your settings/);
		expect(notice).toHaveTextContent("disk is full");
		expect(launch()).toBeChecked();
	});

	it("AC3 edge: a refused change followed by a successful one ends with the switch on", async () => {
		healthyCore();
		await loaded();
		mocked.setSettings.mockRejectedValueOnce("launch at login: no");
		fireEvent.click(launch());
		await screen.findByText(/Could not change the setting/);
		await waitFor(() => expect(launch()).not.toBeChecked());
		mocked.setSettings.mockResolvedValueOnce(
			result(state(), { launch_at_login: true }),
		);
		fireEvent.click(launch());
		await waitFor(() => expect(launch()).toBeChecked());
		expect(mocked.setSettings).toHaveBeenCalledTimes(2);
		expect(mocked.setSettings).toHaveBeenLastCalledWith({
			launch_at_login: true,
		});
	});
});

describe("AC4: changes that arrive from the core", () => {
	it("AC4: an event with a higher revision moves the switch, with no set_settings call", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ launch_at_login: true });
		});
		await waitFor(() => expect(launch()).toBeChecked());
		expect(mocked.setSettings).not.toHaveBeenCalled();
		act(() => {
			core.outsideChange({ launch_at_login: false });
		});
		await waitFor(() => expect(launch()).not.toBeChecked());
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC4: an event with a lower revision is ignored", async () => {
		const core = healthyCore();
		await loaded();
		await waitFor(() => expect(core.listenerCount()).toBe(1));
		act(() => {
			core.outsideChange({ launch_at_login: true });
		});
		await waitFor(() => expect(launch()).toBeChecked());
		act(() => {
			core.emit(result(state(), { launch_at_login: false }, { revision: 0 }));
		});
		expect(launch()).toBeChecked();
	});
});

describe("AC5: role, name, description and keyboard", () => {
	it("AC5: it is a native checkbox with role switch, and the line is its description", async () => {
		healthyCore();
		await loaded();
		const el = launch() as HTMLInputElement;
		expect(el.tagName).toBe("INPUT");
		expect(el.type).toBe("checkbox");
		expect(el).toHaveAccessibleDescription(NOTE);
	});

	it("AC5: it is in the tab order after the pack list's controls", async () => {
		healthyCore();
		await loaded();
		expect(launch()).toBeEnabled();
		expect(launch().tabIndex).toBeGreaterThanOrEqual(0);
		const controls = [
			screen.getByRole("button", { name: "Preview" }),
			screen.getByRole("button", { name: /Import pack/ }),
			screen.getByRole("radio", { name: /Red switches/ }),
		];
		for (const el of controls) {
			expect(
				el.compareDocumentPosition(launch()) & Node.DOCUMENT_POSITION_FOLLOWING,
			).toBeTruthy();
		}
	});

	it("AC5: it can take focus, as Tab and Space need", async () => {
		healthyCore();
		await loaded();
		launch().focus();
		expect(launch()).toHaveFocus();
	});
});

describe("AC6: the switch with Sound off", () => {
	it("AC6: enabled false leaves it enabled, and it still works", async () => {
		healthyCore(withSettings({ enabled: false }));
		await loaded();
		expect(screen.getByRole("switch", { name: "Sound" })).not.toBeChecked();
		expect(launch()).toBeEnabled();
		fireEvent.click(launch());
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({
				launch_at_login: true,
			}),
		);
	});
});

describe("AC7: the permission screen", () => {
	it.each(["needed", "restart_needed"] as const)(
		"AC7: with the permission %s the switch is not shown",
		async (permission) => {
			healthyCore(state({ permission }));
			render(<App />);
			await screen.findByRole("heading", {
				name: "Let ClickClak hear your keys",
			});
			expect(
				screen.queryByRole("switch", { name: "Launch at login" }),
			).not.toBeInTheDocument();
			expect(screen.queryByText(NOTE)).not.toBeInTheDocument();
		},
	);
});
