// Feature 08 (macOS permission onboarding), the window's half: AC11 to AC20.
// The core is mocked at `src/api`. `getSettings` gives the opening state;
// `permissionStatus` is driven with `core.setPermission` and fake timers.
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as api from "./api";
import type { PermissionState } from "./api/types";
import { healthyCore, state } from "./test/fixtures";

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

const HEADING = "Let Clatterkey hear your keys";
const WAITING = "Waiting for permission…";
const COULD_NOT_OPEN =
	"System Settings could not be opened. Open it yourself: Privacy & Security, then Input Monitoring.";
const RESTART_TEXT = "Permission granted. Restart Clatterkey to finish.";
const WARNING =
	"Clatterkey cannot hear your keys yet. It needs the Input Monitoring permission.";

beforeEach(() => {
	vi.resetAllMocks();
	vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
	vi.useRealTimers();
});

/** Renders the window with the core opening in `permission`. */
async function open(permission: PermissionState, over = {}) {
	const core = healthyCore(state({ permission, ...over }));
	render(<App />);
	await waitFor(() =>
		expect(
			screen.queryByRole("switch", { name: "Sound" }) ??
				screen.queryByRole("heading", { name: HEADING }),
		).not.toBeNull(),
	);
	return core;
}

/** Lets the once-a-second poll run. */
async function poll(seconds = 1) {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(seconds * 1000);
	});
}

const heading = () => screen.queryByRole("heading", { name: HEADING });
const sound = () => screen.queryByRole("switch", { name: "Sound" });
const openButton = () =>
	screen.queryByRole("button", { name: "Open System Settings" });
const laterButton = () =>
	screen.queryByRole("button", { name: "Set up later" });
const setUpButton = () => screen.queryByRole("button", { name: "Set up" });

function statusWith(text: string) {
	return screen
		.queryAllByRole("status")
		.find((el) => el.textContent?.includes(text));
}

describe("AC11: the permission screen", () => {
	it("AC11: needed shows the heading, both paragraphs, the three steps and the two buttons", async () => {
		await open("needed");
		expect(heading()).toBeInTheDocument();
		expect(
			screen.getByText(
				"macOS only tells an app that a key was pressed if you allow it under Input Monitoring. Until you do, the app is silent.",
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				"The app only notices that a key went down, so it can play a sound. It never records, stores or sends what you type.",
			),
		).toBeInTheDocument();
		const steps = within(screen.getByRole("list")).getAllByRole("listitem");
		expect(steps.map((s) => s.textContent)).toEqual([
			"Click Open System Settings.",
			"Turn on Clatterkey in the Input Monitoring list.",
			"Come back here. The sound starts by itself.",
		]);
		expect(openButton()).toBeInTheDocument();
		expect(laterButton()).toBeInTheDocument();
	});

	it("AC11: the Sound switch, volume slider and pack list are not in the page", async () => {
		await open("needed");
		expect(sound()).toBeNull();
		expect(screen.queryByRole("slider")).toBeNull();
		expect(screen.queryByRole("radiogroup")).toBeNull();
		expect(screen.queryByRole("radio")).toBeNull();
	});

	it("AC11 edge: the title stays and the controls never flash before the screen", async () => {
		healthyCore(state({ permission: "needed" }));
		render(<App />);
		expect(
			screen.getByRole("heading", { name: "Clatterkey", level: 1 }),
		).toBeInTheDocument();
		await screen.findByRole("heading", { name: HEADING });
		expect(sound()).toBeNull();
	});
});

describe("AC12: nothing to show when granted or not needed", () => {
	it.each(["granted", "not_needed"] as const)(
		"AC12: %s shows the settings controls and no permission screen or notice",
		async (permission) => {
			await open(permission);
			expect(sound()).toBeInTheDocument();
			expect(
				screen.getByRole("slider", { name: "Volume" }),
			).toBeInTheDocument();
			expect(heading()).toBeNull();
			expect(openButton()).toBeNull();
			expect(screen.queryByText(WARNING)).toBeNull();
			expect(setUpButton()).toBeNull();
		},
	);

	it("AC12: with not_needed permissionStatus is never called", async () => {
		await open("not_needed");
		await poll(5);
		expect(mocked.permissionStatus).not.toHaveBeenCalled();
		expect(mocked.audioStatus).toHaveBeenCalled();
	});
});

describe("AC13: Open System Settings", () => {
	it("AC13: pressing it calls the command once and shows the waiting line; again calls it again", async () => {
		await open("needed");
		expect(statusWith(WAITING)).toBeUndefined();
		fireEvent.click(openButton() as HTMLElement);
		await waitFor(() => expect(statusWith(WAITING)).toBeDefined());
		expect(mocked.openPermissionSettings).toHaveBeenCalledTimes(1);
		fireEvent.click(openButton() as HTMLElement);
		await waitFor(() =>
			expect(mocked.openPermissionSettings).toHaveBeenCalledTimes(2),
		);
		expect(statusWith(WAITING)).toBeDefined();
	});

	it("AC13 edge: pressed twice quickly it calls twice and shows one waiting line", async () => {
		await open("needed");
		const button = openButton() as HTMLElement;
		fireEvent.click(button);
		fireEvent.click(button);
		await waitFor(() => expect(statusWith(WAITING)).toBeDefined());
		expect(mocked.openPermissionSettings).toHaveBeenCalledTimes(2);
		expect(screen.getAllByText(WAITING)).toHaveLength(1);
	});
});

describe("AC14: System Settings could not be opened", () => {
	it("AC14: a failing command shows the line as an alert and the buttons still work", async () => {
		await open("needed");
		mocked.openPermissionSettings.mockRejectedValue("no pane");
		fireEvent.click(openButton() as HTMLElement);
		const alert = await screen.findByRole("alert");
		expect(alert).toHaveTextContent(COULD_NOT_OPEN);
		expect(openButton()).toBeEnabled();
		expect(laterButton()).toBeEnabled();

		fireEvent.click(openButton() as HTMLElement);
		await waitFor(() =>
			expect(mocked.openPermissionSettings).toHaveBeenCalledTimes(2),
		);
		fireEvent.click(laterButton() as HTMLElement);
		expect(await screen.findByRole("switch", { name: "Sound" })).toBeVisible();
	});
});

describe("AC15: the grant", () => {
	it("AC15: a polled granted replaces the screen with the controls, showing the values in force", async () => {
		const core = await open("needed");
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(sound()).toBeInTheDocument());
		expect(heading()).toBeNull();
		expect(mocked.getSettings).toHaveBeenCalledTimes(1); // no reload
		expect(screen.getByRole("slider", { name: "Volume" })).toHaveValue("80");
		expect(screen.getByRole("radio", { name: /Red switches/ })).toBeChecked();
		expect(sound()).toBeChecked();
	});

	it("AC15: focus is on the Sound switch", async () => {
		const core = await open("needed");
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(sound()).toBeInTheDocument());
		await waitFor(() => expect(sound()).toHaveFocus());
	});

	it("AC15 edge: permissionStatus rejecting leaves the screen as it was", async () => {
		await open("needed");
		mocked.permissionStatus.mockRejectedValue("boom");
		await poll(3);
		expect(heading()).toBeInTheDocument();
		expect(sound()).toBeNull();
		expect(mocked.permissionStatus).toHaveBeenCalled();
	});
});

describe("AC16: restart needed", () => {
	it("AC16: restart_needed shows the restart text and Restart now, not Open System Settings", async () => {
		await open("restart_needed");
		expect(statusWith(RESTART_TEXT)).toBeDefined();
		expect(
			screen.getByRole("button", { name: "Restart now" }),
		).toBeInTheDocument();
		expect(openButton()).toBeNull();
		expect(sound()).toBeNull();
	});

	it("AC16: pressing Restart now calls the restart command once", async () => {
		await open("restart_needed");
		fireEvent.click(screen.getByRole("button", { name: "Restart now" }));
		await waitFor(() => expect(mocked.restartApp).toHaveBeenCalledTimes(1));
	});

	it("AC16: a polled restart_needed changes the screen to the restart text", async () => {
		const core = await open("needed");
		core.setPermission("restart_needed");
		await poll();
		await waitFor(() => expect(statusWith(RESTART_TEXT)).toBeDefined());
		expect(openButton()).toBeNull();
		expect(
			screen.getByRole("button", { name: "Restart now" }),
		).toBeInTheDocument();
	});

	it("AC16 edge: from restart_needed a later grant shows the controls", async () => {
		const core = await open("restart_needed");
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(sound()).toBeInTheDocument());
		expect(screen.queryByRole("button", { name: "Restart now" })).toBeNull();
	});
});

describe("AC17: Set up later", () => {
	it("AC17: shows the controls with the warning notice first, a Set up button and no dismiss button", async () => {
		await open("needed", { audio: "no_device" });
		fireEvent.click(laterButton() as HTMLElement);
		await waitFor(() => expect(sound()).toBeInTheDocument());
		expect(heading()).toBeNull();

		const strip = document.querySelector(".notices");
		expect(strip).not.toBeNull();
		const first = strip?.firstElementChild as HTMLElement;
		expect(first).toHaveTextContent(WARNING);
		expect(
			within(first).getByRole("button", { name: "Set up" }),
		).toBeInTheDocument();
		expect(within(first).queryByRole("button", { name: "Dismiss" })).toBeNull();
		// The audio notice is still there, after it.
		expect(strip).toHaveTextContent(/No audio output device found/);
	});

	it("AC17: changing a setting calls set_settings as usual", async () => {
		await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		const sw = await screen.findByRole("switch", { name: "Sound" });
		fireEvent.click(sw);
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ enabled: false }),
		);
	});
});

describe("AC18: back to the screen, and the grant after Set up later", () => {
	it("AC18: Set up returns to the permission screen", async () => {
		await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		await screen.findByRole("switch", { name: "Sound" });
		fireEvent.click(setUpButton() as HTMLElement);
		await waitFor(() => expect(heading()).toBeInTheDocument());
		expect(sound()).toBeNull();
		expect(screen.queryByText(WARNING)).toBeNull();
	});

	it("AC18: a grant removes the notice and the controls stay", async () => {
		const core = await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		await screen.findByRole("switch", { name: "Sound" });
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(screen.queryByText(WARNING)).toBeNull());
		expect(setUpButton()).toBeNull();
		expect(sound()).toBeInTheDocument();
		expect(heading()).toBeNull();
	});

	it("AC18 edge: a grant during Set up later does not move focus", async () => {
		const core = await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		await screen.findByRole("switch", { name: "Sound" });
		const slider = screen.getByRole("slider", { name: "Volume" });
		act(() => slider.focus());
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(screen.queryByText(WARNING)).toBeNull());
		expect(slider).toHaveFocus();
	});

	it("AC18 edge: restart_needed with Set up later shows the notice, and Set up shows the restart text", async () => {
		const core = await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		await screen.findByRole("switch", { name: "Sound" });
		core.setPermission("restart_needed");
		await poll();
		await waitFor(() => expect(setUpButton()).toBeInTheDocument());
		expect(screen.getByText(WARNING)).toBeInTheDocument();
		expect(sound()).toBeInTheDocument();
		fireEvent.click(setUpButton() as HTMLElement);
		await waitFor(() => expect(statusWith(RESTART_TEXT)).toBeDefined());
		expect(
			screen.getByRole("button", { name: "Restart now" }),
		).toBeInTheDocument();
	});
});

describe("AC19: the permission is removed", () => {
	it("AC19: from granted, a polled needed shows the permission screen", async () => {
		const core = await open("granted");
		expect(sound()).toBeInTheDocument();
		core.setPermission("needed");
		await poll();
		await waitFor(() => expect(heading()).toBeInTheDocument());
		expect(sound()).toBeNull();
	});

	it("AC19 edge: the screen then takes focus on its heading", async () => {
		const core = await open("granted");
		core.setPermission("needed");
		await poll();
		await waitFor(() => expect(heading()).toBeInTheDocument());
		await waitFor(() => expect(heading()).toHaveFocus());
	});

	it("AC19 edge: with Set up later chosen, a polled needed after a grant shows the notice", async () => {
		const core = await open("needed");
		fireEvent.click(laterButton() as HTMLElement);
		await screen.findByRole("switch", { name: "Sound" });
		core.setPermission("granted");
		await poll();
		await waitFor(() => expect(screen.queryByText(WARNING)).toBeNull());
		core.setPermission("needed");
		await poll();
		await waitFor(() =>
			expect(screen.queryByText(WARNING)).toBeInTheDocument(),
		);
		expect(sound()).toBeInTheDocument();
	});
});

describe("AC20: accessibility of the screen", () => {
	it("AC20: the heading has a heading role and the steps are an ordered list of three", async () => {
		await open("needed");
		expect(screen.getByRole("heading", { name: HEADING })).toBeInTheDocument();
		const list = screen.getByRole("list");
		expect(list.tagName).toBe("OL");
		expect(within(list).getAllByRole("listitem")).toHaveLength(3);
	});

	it("AC20: both buttons are in reading order and none is taken out of the Tab order", async () => {
		await open("needed");
		const buttons = screen.getAllByRole("button");
		expect(buttons.map((b) => b.textContent)).toEqual([
			"Open System Settings",
			"Set up later",
		]);
		for (const b of buttons) {
			expect(b).not.toBeDisabled();
			expect(Number(b.getAttribute("tabindex") ?? 0)).toBe(0);
		}
		const [first, second] = buttons;
		expect(
			first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("AC20: the waiting text has the status role", async () => {
		await open("needed");
		fireEvent.click(openButton() as HTMLElement);
		await waitFor(() => expect(statusWith(WAITING)).toBeDefined());
	});

	it("AC20: the restart text has the status role", async () => {
		await open("restart_needed");
		expect(statusWith(RESTART_TEXT)).toBeDefined();
	});
});

describe("AC20: one status element that stays mounted", () => {
	it("AC20: the status element is present and empty before the button is pressed", async () => {
		await open("needed");
		const statuses = screen.getAllByRole("status");
		expect(statuses.some((el) => el.textContent === "")).toBe(true);
		expect(statusWith(WAITING)).toBeUndefined();
	});

	it("AC20: the same status node holds the restart text after the screen changes to the restart form", async () => {
		const core = await open("needed");
		const before = screen
			.getAllByRole("status")
			.find((el) => el.textContent === "") as HTMLElement;
		expect(before).toBeDefined();
		core.setPermission("restart_needed");
		await poll();
		await waitFor(() => expect(statusWith(RESTART_TEXT)).toBeDefined());
		expect(statusWith(RESTART_TEXT)).toBe(before);
		expect(before.isConnected).toBe(true);
	});

	it("AC20: the same status node goes from the waiting text to the restart text", async () => {
		const core = await open("needed");
		fireEvent.click(openButton() as HTMLElement);
		await waitFor(() => expect(statusWith(WAITING)).toBeDefined());
		const waiting = statusWith(WAITING) as HTMLElement;
		core.setPermission("restart_needed");
		await poll();
		await waitFor(() => expect(statusWith(RESTART_TEXT)).toBeDefined());
		expect(statusWith(RESTART_TEXT)).toBe(waiting);
	});
});
