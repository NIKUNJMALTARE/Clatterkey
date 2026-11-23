import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
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

beforeEach(() => {
	vi.resetAllMocks();
	healthyCore();
});

describe("AC11: roles, names and keyboard", () => {
	it("AC11: the switch, the slider and the pack list have a role and an accessible name", async () => {
		render(<App />);
		expect(
			await screen.findByRole("switch", { name: "Sound" }),
		).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "Volume" })).toBeInTheDocument();
		const group = screen.getByRole("radiogroup", { name: "Sound pack" });
		expect(within(group).getAllByRole("radio")).toHaveLength(4);
	});

	it("AC11: every radio is named after its pack", async () => {
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		for (const name of [
			"Blue switches",
			"Brown switches",
			"Red switches",
			"Typewriter",
		]) {
			expect(
				screen.getByRole("radio", { name: new RegExp(name) }),
			).toBeInTheDocument();
		}
	});

	it("AC11: the controls are reachable with Tab (none is taken out of the tab order)", async () => {
		render(<App />);
		const sw = await screen.findByRole("switch", { name: "Sound" });
		const slider = screen.getByRole("slider", { name: "Volume" });
		const selected = screen.getByRole("radio", { name: /Red switches/ });
		for (const el of [sw, slider, selected]) {
			expect(el).toBeEnabled();
			expect(el.tabIndex).toBeGreaterThanOrEqual(0);
		}
		// The list is one tab stop: native radios of one group share a name.
		const radios = screen.getAllByRole("radio") as HTMLInputElement[];
		const names = new Set(radios.map((r) => r.name));
		expect(names.size).toBe(1);
		expect([...names][0]).not.toBe("");
	});

	it("AC11: the switch is a native checkbox that works from the keyboard", async () => {
		render(<App />);
		const sw = (await screen.findByRole("switch", {
			name: "Sound",
		})) as HTMLInputElement;
		expect(sw.tagName).toBe("INPUT");
		expect(sw.type).toBe("checkbox");
	});

	it("AC11: the slider is a native range input, which gives the arrow, Page and Home/End keys", async () => {
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		const slider = screen.getByRole("slider", {
			name: "Volume",
		}) as HTMLInputElement;
		expect(slider.tagName).toBe("INPUT");
		expect(slider.type).toBe("range");
	});

	it("AC11: the slider's value is announced as a percentage", async () => {
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		expect(screen.getByRole("slider", { name: "Volume" })).toHaveAttribute(
			"aria-valuetext",
			"80%",
		);
		fireEvent.change(screen.getByRole("slider", { name: "Volume" }), {
			target: { value: "35" },
		});
		expect(screen.getByRole("slider", { name: "Volume" })).toHaveAttribute(
			"aria-valuetext",
			"35%",
		);
	});

	it("AC11: errors are alerts and the other notices are status messages, not dialogs", async () => {
		vi.resetAllMocks();
		healthyCore(
			state({
				audio: "no_device",
				startup: {
					settings_unreadable: true,
					replaced_fields: [],
					unusable_pack: null,
				},
			}),
		);
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
		const noticeCount =
			screen.queryAllByRole("alert").length +
			screen.queryAllByRole("status").length;
		expect(noticeCount).toBeGreaterThanOrEqual(2);
	});

	it("AC11: the pack list is a group with the legend Sound pack, scrolled as one list", async () => {
		render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		const group = screen.getByRole("radiogroup", { name: "Sound pack" });
		expect(group).toBeInTheDocument();
	});
});
