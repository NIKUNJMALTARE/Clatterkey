// Feature 13 (the app name), the window's half: AC1 to AC3. The core is
// mocked at `src/api`. The window states already covered by the other App
// tests are rendered here and none of them may show the old name.
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import type { PackInfo } from "./api/types";
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

// Spelled from parts so that no file of the repository holds the old name
// (see scripts/tests/old-name.test.mjs).
const OLD_NAME = ["Mechanical", "Key", "Sounds"].join(" ");
const OLD_PATTERN = new RegExp(OLD_NAME.split(" ").join("\\s*"), "i");

beforeEach(() => {
	vi.resetAllMocks();
});

const NEEDED = state({ permission: "needed" });

describe("AC1: the heading", () => {
	it("AC1: the settings view's heading reads ClickClak", async () => {
		healthyCore();
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		expect(
			screen.getByRole("heading", { name: "ClickClak", level: 1 }),
		).toBeInTheDocument();
	});

	it("AC1: the heading is there while loading too", () => {
		healthyCore();
		render(<App />);
		expect(
			screen.getByRole("heading", { name: "ClickClak", level: 1 }),
		).toBeInTheDocument();
	});
});

describe("AC2: the permission screen names ClickClak", () => {
	it("AC2: needed shows the heading and the step", async () => {
		healthyCore(NEEDED);
		render(<App />);
		expect(
			await screen.findByRole("heading", {
				name: "Let ClickClak hear your keys",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByText("Turn on ClickClak in the Input Monitoring list."),
		).toBeInTheDocument();
	});

	it("AC2: restart_needed says Permission granted. Restart ClickClak to finish.", async () => {
		healthyCore(state({ permission: "restart_needed" }));
		render(<App />);
		expect(
			await screen.findByText(
				"Permission granted. Restart ClickClak to finish.",
			),
		).toBeInTheDocument();
	});

	it("AC2: Set up later leaves the reminder naming ClickClak", async () => {
		healthyCore(NEEDED);
		render(<App />);
		await screen.findByRole("heading", {
			name: "Let ClickClak hear your keys",
		});
		fireEvent.click(screen.getByRole("button", { name: "Set up later" }));
		expect(
			await screen.findByText(
				"ClickClak cannot hear your keys yet. It needs the Input Monitoring permission.",
			),
		).toBeInTheDocument();
	});
});

describe("AC3: the old name appears nowhere in the window", () => {
	async function shown(over: Parameters<typeof state>[0], packs?: PackInfo[]) {
		const core = healthyCore(state(over), packs);
		render(<App />);
		await waitFor(() => {
			expect(
				screen.queryByRole("switch", { name: "Sound" }) ??
					screen.queryByRole("heading", { name: /hear your keys/ }),
			).not.toBeNull();
		});
		return core;
	}

	function expectNoOldName() {
		expect(document.body.textContent ?? "").not.toMatch(OLD_PATTERN);
		for (const el of document.body.querySelectorAll("[aria-label],[title]")) {
			expect(el.getAttribute("aria-label") ?? "").not.toMatch(OLD_PATTERN);
			expect(el.getAttribute("title") ?? "").not.toMatch(OLD_PATTERN);
		}
	}

	it("AC3: while loading", () => {
		healthyCore();
		render(<App />);
		expectNoOldName();
	});

	it("AC3: the settings view with the bundled packs and their authors", async () => {
		await shown({});
		expect(
			screen.getAllByText(/ClickClak contributors/).length,
		).toBeGreaterThan(0);
		expectNoOldName();
	});

	it("AC3: with sound off", async () => {
		await shown({ settings: { ...state().settings, enabled: false } });
		expectNoOldName();
	});

	it("AC3: with no packs", async () => {
		await shown({}, []);
		expectNoOldName();
	});

	it("AC3: on the permission screen", async () => {
		await shown({ permission: "needed" });
		expectNoOldName();
	});

	it("AC3: on the restart text", async () => {
		await shown({ permission: "restart_needed" });
		expectNoOldName();
	});

	it("AC3: with the reminder after Set up later", async () => {
		await shown({ permission: "needed" });
		fireEvent.click(screen.getByRole("button", { name: "Set up later" }));
		await screen.findByText(/cannot hear your keys yet/);
		expectNoOldName();
	});

	it("AC3: with the audio output unavailable and a startup problem", async () => {
		await shown({
			audio: "no_device",
			startup: {
				settings_unreadable: true,
				replaced_fields: [],
				unusable_pack: null,
			},
		});
		expectNoOldName();
	});
});
