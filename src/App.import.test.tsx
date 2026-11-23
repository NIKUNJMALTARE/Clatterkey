// Feature 10: importing and removing packs in the settings window (AC1 to
// AC10). The core is mocked at the `src/api` seam. AC11 is the browser
// check and AC29 the manual checklist.
//
// Assumed from the plan: `api.importPack(): Promise<ImportResult | null>` and
// `api.removePack(id): Promise<RemoveResult>`; `PackInfo.imported`.
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as api from "./api";
import type { ImportResult, PackInfo, RemoveResult } from "./api/types";
import {
	healthyCore,
	importedPack,
	PACKS,
	result,
	state,
} from "./test/fixtures";

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

const MINE = importedPack("my-clicks", "My clicks");
const LOUD = importedPack("loud", "Loud Board", {
	author: "Someone",
	license: "CC0-1.0",
});
const WITH_IMPORTED: PackInfo[] = [...PACKS, MINE, LOUD];

async function loaded() {
	render(<App />);
	return screen.findByRole("switch", { name: "Sound" });
}

const importButton = () => screen.getByRole("button", { name: "Import pack…" });
const previewButton = () => screen.getByRole("button", { name: "Preview" });
const radio = (name: RegExp | string) => screen.getByRole("radio", { name });
const removeButton = (name: string) =>
	screen.getByRole("button", { name: `Remove ${name}` });
const group = (name: string) =>
	screen.getByRole("group", { name: `Remove ${name}?` });
const alerts = () => screen.queryAllByRole("alert");

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

/** What the core answers when My clicks was imported and is now playing. */
function imported(
	over: Partial<ImportResult> = {},
	extra: Record<string, unknown> = {},
): ImportResult {
	return {
		pack: MINE,
		replaced: false,
		packs: [...PACKS, MINE],
		change: result(state(), { pack_id: "my-clicks" }, extra),
		...over,
	};
}

function removed(
	packs: PackInfo[],
	change: RemoveResult["change"],
): RemoveResult {
	return { packs, change };
}

/** The window with My clicks and Loud Board listed and `playing` chosen. */
async function withImported(playing = "red") {
	healthyCore(
		state({
			settings: { ...state().settings, pack_id: playing },
			playing_pack: playing,
		}),
		WITH_IMPORTED,
	);
	await loaded();
}

/** The polite live region holding `text`, if any. */
function announced(text: string) {
	const node = screen.queryByText(text);
	return node?.closest('[aria-live="polite"]') ?? null;
}

describe("AC1: the button and the rows", () => {
	it("AC1: Import pack… is on the Sound pack heading line, before Preview", async () => {
		healthyCore();
		await loaded();
		const heading = screen.getByRole("heading", { name: "Sound pack" });
		const button = importButton();
		expect(
			heading.compareDocumentPosition(button) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			button.compareDocumentPosition(previewButton()) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		const list = screen.getByRole("radiogroup", { name: "Sound pack" });
		expect(
			previewButton().compareDocumentPosition(list) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(heading.parentElement).toContainElement(button);
		expect(heading.parentElement).toContainElement(previewButton());
	});

	it("AC1: with no packs at all, Import pack… and Preview are still shown", async () => {
		healthyCore(state({ playing_pack: null }), []);
		await loaded();
		expect(screen.getByText(/No sound packs were found/)).toBeInTheDocument();
		expect(importButton()).toBeInTheDocument();
		expect(previewButton()).toBeInTheDocument();
	});

	it("AC1: imported rows say Imported · and have a Remove button; bundled rows have neither", async () => {
		await withImported();
		const mine = radio(/My clicks/);
		expect(mine.closest("label")).toHaveTextContent(
			"Imported · Licence not stated",
		);
		expect(radio(/Loud Board/).closest("label")).toHaveTextContent(
			"Imported · Someone · CC0-1.0",
		);
		expect(removeButton("My clicks")).toBeInTheDocument();
		expect(removeButton("Loud Board")).toBeInTheDocument();
		for (const name of [
			"Blue switches",
			"Brown switches",
			"Red switches",
			"Typewriter",
		]) {
			const label = radio(new RegExp(name)).closest("label");
			expect(label).not.toHaveTextContent("Imported");
			expect(
				screen.queryByRole("button", { name: `Remove ${name}` }),
			).not.toBeInTheDocument();
		}
		expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(2);
	});

	it("AC1: with no imported packs the list has no Remove button", async () => {
		healthyCore();
		await loaded();
		expect(
			screen.queryByRole("button", { name: /^Remove / }),
		).not.toBeInTheDocument();
		expect(screen.queryByText(/Imported ·/)).not.toBeInTheDocument();
	});
});

describe("AC2: pressing Import pack…", () => {
	it("AC2: the core's importPack is called once with no arguments", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(null);
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(1));
		expect(mocked.importPack).toHaveBeenCalledWith();
	});

	it("AC2: while it is pending, more presses call nothing and the button is busy; it works again afterwards", async () => {
		healthyCore();
		const pending = deferred<ImportResult | null>();
		mocked.importPack.mockReturnValueOnce(pending.promise);
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(importButton()).toHaveAttribute("aria-busy", "true"),
		);
		fireEvent.click(importButton());
		fireEvent.click(importButton());
		expect(mocked.importPack).toHaveBeenCalledTimes(1);
		// It stays focusable, and the rest of the window stays usable.
		expect(importButton()).not.toBeDisabled();
		expect(radio(/Blue switches/)).not.toBeDisabled();

		await act(async () => pending.resolve(null));
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		mocked.importPack.mockResolvedValueOnce(null);
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(2));
	});

	it("AC2: after a rejection the button works again", async () => {
		healthyCore();
		const pending = deferred<ImportResult | null>();
		mocked.importPack.mockReturnValueOnce(pending.promise);
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(importButton()).toHaveAttribute("aria-busy", "true"),
		);
		await act(async () => pending.reject('Could not import "x": nope'));
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		mocked.importPack.mockResolvedValueOnce(null);
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(2));
	});
});

describe("AC3: an import that succeeds", () => {
	it("AC3: the list comes from the result, the new pack is chosen and announced, and no error shows", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(imported());
		await loaded();
		expect(radio(/Red switches/)).toBeChecked();
		expect(
			screen.queryByRole("radio", { name: /My clicks/ }),
		).not.toBeInTheDocument();

		fireEvent.click(importButton());

		await waitFor(() => expect(radio(/My clicks/)).toBeChecked());
		expect(radio(/Red switches/)).not.toBeChecked();
		expect(removeButton("My clicks")).toBeInTheDocument();
		expect(announced("Imported My clicks.")).not.toBeNull();
		expect(alerts()).toHaveLength(0);
		// The choice is the core's: the window asks for no second change.
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC3: the list shows exactly the packs of the result, so a replaced pack is listed once", async () => {
		healthyCore(state(), [...PACKS, MINE]);
		mocked.importPack.mockResolvedValue(imported({ replaced: true }));
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() => expect(radio(/My clicks/)).toBeChecked());
		expect(screen.getAllByRole("radio", { name: /My clicks/ })).toHaveLength(1);
	});

	it("AC3: a second identical announcement is rendered again", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(imported());
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(announced("Imported My clicks.")).not.toBeNull(),
		);
		const first = screen.getByText("Imported My clicks.");
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(2));
		await waitFor(() => expect(first.isConnected).toBe(false));
		expect(announced("Imported My clicks.")).not.toBeNull();
	});

	it("AC3: the announcement is not an alert and not a status notice", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(imported());
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(announced("Imported My clicks.")).not.toBeNull(),
		);
		expect(
			screen.getByText("Imported My clicks.").closest('[role="status"]'),
		).toBeNull();
		expect(alerts()).toHaveLength(0);
	});

	it("AC3 edge: an import that resolves after the window has gone sets nothing and does not throw", async () => {
		healthyCore();
		const pending = deferred<ImportResult | null>();
		mocked.importPack.mockReturnValueOnce(pending.promise);
		const errors = vi.spyOn(console, "error").mockImplementation(() => {});
		const view = render(<App />);
		await screen.findByRole("switch", { name: "Sound" });
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(1));
		view.unmount();
		await act(async () => pending.resolve(imported()));
		expect(errors).not.toHaveBeenCalled();
		errors.mockRestore();
	});
});

describe("AC4: cancelling", () => {
	it("AC4: a null result changes nothing and shows nothing", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(null);
		await loaded();
		const before = screen.getAllByRole("radio").map((r) => r.id || r.outerHTML);
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(1));
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		expect(
			screen.getAllByRole("radio").map((r) => r.id || r.outerHTML),
		).toEqual(before);
		expect(radio(/Red switches/)).toBeChecked();
		expect(alerts()).toHaveLength(0);
		expect(screen.queryByText(/Imported /)).not.toBeInTheDocument();
		expect(mocked.setSettings).not.toHaveBeenCalled();
	});

	it("AC4: cancelling leaves an earlier notice as it was", async () => {
		healthyCore(
			state({
				startup: {
					settings_unreadable: true,
					replaced_fields: [],
					unusable_pack: null,
				},
			}),
		);
		mocked.importPack.mockResolvedValue(null);
		await loaded();
		const before = alerts().length + screen.queryAllByRole("status").length;
		expect(before).toBeGreaterThan(0);
		fireEvent.click(importButton());
		await waitFor(() => expect(mocked.importPack).toHaveBeenCalledTimes(1));
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		expect(alerts().length + screen.queryAllByRole("status").length).toBe(
			before,
		);
	});
});

describe("AC5: an import that is refused", () => {
	const REASON =
		'Could not import "Old Board": "thock 3.wav": longer than 3 seconds';

	it("AC5: the reason is shown as given, the list and selection stay, and it can be dismissed", async () => {
		healthyCore();
		mocked.importPack.mockRejectedValue(REASON);
		await loaded();
		fireEvent.click(importButton());

		const notice = await screen.findByRole("alert");
		expect(notice).toHaveTextContent(REASON);
		expect(notice.textContent).not.toMatch(
			/Could not import.*Could not import/,
		);
		expect(screen.getAllByRole("radio")).toHaveLength(4);
		expect(radio(/Red switches/)).toBeChecked();
		expect(screen.queryByText(/^Imported /)).not.toBeInTheDocument();

		fireEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
		expect(
			screen.queryByText(REASON, { exact: false }),
		).not.toBeInTheDocument();
	});

	it("AC5: a rejection with an Error shows its message", async () => {
		healthyCore();
		mocked.importPack.mockRejectedValue(new Error(REASON));
		await loaded();
		fireEvent.click(importButton());
		expect(await screen.findByRole("alert")).toHaveTextContent(REASON);
	});

	it("AC5: starting another import removes the notice", async () => {
		healthyCore();
		const pending = deferred<ImportResult | null>();
		mocked.importPack
			.mockRejectedValueOnce(REASON)
			.mockReturnValueOnce(pending.promise);
		await loaded();
		fireEvent.click(importButton());
		await screen.findByText(REASON, { exact: false });
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(
				screen.queryByText(REASON, { exact: false }),
			).not.toBeInTheDocument(),
		);
		await act(async () => pending.resolve(null));
	});

	it("AC5: a later successful import leaves no old notice behind", async () => {
		healthyCore();
		mocked.importPack
			.mockRejectedValueOnce(REASON)
			.mockResolvedValueOnce(imported());
		await loaded();
		fireEvent.click(importButton());
		await screen.findByText(REASON, { exact: false });
		await waitFor(() =>
			expect(importButton()).not.toHaveAttribute("aria-busy", "true"),
		);
		fireEvent.click(importButton());
		await waitFor(() => expect(radio(/My clicks/)).toBeChecked());
		expect(
			screen.queryByText(REASON, { exact: false }),
		).not.toBeInTheDocument();
	});
});

describe("AC6: the question in the row", () => {
	it("AC6: Remove asks in the row and the core is not called", async () => {
		await withImported();
		fireEvent.click(removeButton("My clicks"));
		const asking = group("My clicks");
		expect(asking).toHaveTextContent("Remove?");
		expect(
			within(asking).getByRole("button", { name: "Remove" }),
		).toBeInTheDocument();
		expect(
			within(asking).getByRole("button", { name: "Cancel" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Remove My clicks" }),
		).not.toBeInTheDocument();
		expect(mocked.removePack).not.toHaveBeenCalled();
	});

	it("AC6: Cancel restores the row with focus on its Remove button", async () => {
		await withImported();
		fireEvent.click(removeButton("My clicks"));
		fireEvent.click(
			within(group("My clicks")).getByRole("button", { name: "Cancel" }),
		);
		expect(
			screen.queryByRole("group", { name: "Remove My clicks?" }),
		).not.toBeInTheDocument();
		expect(removeButton("My clicks")).toHaveFocus();
		expect(mocked.removePack).not.toHaveBeenCalled();
	});

	it("AC6: Escape restores the row with focus on its Remove button", async () => {
		await withImported();
		fireEvent.click(removeButton("My clicks"));
		const cancel = within(group("My clicks")).getByRole("button", {
			name: "Cancel",
		});
		fireEvent.keyDown(cancel, { key: "Escape" });
		expect(
			screen.queryByRole("group", { name: "Remove My clicks?" }),
		).not.toBeInTheDocument();
		expect(removeButton("My clicks")).toHaveFocus();
		expect(mocked.removePack).not.toHaveBeenCalled();
	});

	it("AC6: asking on another row moves the question there; only one row asks", async () => {
		await withImported();
		fireEvent.click(removeButton("My clicks"));
		fireEvent.click(removeButton("Loud Board"));
		expect(
			screen.queryByRole("group", { name: "Remove My clicks?" }),
		).not.toBeInTheDocument();
		expect(group("Loud Board")).toBeInTheDocument();
		expect(
			screen.getAllByRole("group", { name: /^Remove .*\?$/ }),
		).toHaveLength(1);
		expect(removeButton("My clicks")).toBeInTheDocument();
		expect(mocked.removePack).not.toHaveBeenCalled();
	});

	it("AC6: the question is not inside the radio's label", async () => {
		await withImported();
		fireEvent.click(removeButton("My clicks"));
		expect(group("My clicks").closest("label")).toBeNull();
	});
});

describe("AC7: confirming a removal", () => {
	it("AC7: removePack is called once with the id; the row goes, the selection follows, focus moves, and it is announced", async () => {
		await withImported("my-clicks");
		expect(radio(/My clicks/)).toBeChecked();
		const answer = removed(
			[...PACKS, LOUD],
			result(state(), { pack_id: "brown" }),
		);
		mocked.removePack.mockResolvedValue(answer);

		fireEvent.click(removeButton("My clicks"));
		fireEvent.click(
			within(group("My clicks")).getByRole("button", { name: "Remove" }),
		);

		await waitFor(() =>
			expect(
				screen.queryByRole("radio", { name: /My clicks/ }),
			).not.toBeInTheDocument(),
		);
		expect(mocked.removePack).toHaveBeenCalledTimes(1);
		expect(mocked.removePack).toHaveBeenCalledWith("my-clicks");
		expect(radio(/Brown switches/)).toBeChecked();
		expect(removeButton("Loud Board")).toBeInTheDocument();
		await waitFor(() => expect(importButton()).toHaveFocus());
		expect(announced("Removed My clicks.")).not.toBeNull();
		expect(alerts()).toHaveLength(0);
	});

	it("AC7: removing a pack that is not playing leaves the selection alone", async () => {
		await withImported("red");
		mocked.removePack.mockResolvedValue(removed([...PACKS, MINE], null));
		fireEvent.click(removeButton("Loud Board"));
		fireEvent.click(
			within(group("Loud Board")).getByRole("button", { name: "Remove" }),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("radio", { name: /Loud Board/ }),
			).not.toBeInTheDocument(),
		);
		expect(mocked.removePack).toHaveBeenCalledWith("loud");
		expect(radio(/Red switches/)).toBeChecked();
		expect(announced("Removed Loud Board.")).not.toBeNull();
		await waitFor(() => expect(importButton()).toHaveFocus());
	});

	it("AC7: confirming twice quickly calls the core once", async () => {
		await withImported("red");
		const pending = deferred<RemoveResult>();
		mocked.removePack.mockReturnValueOnce(pending.promise);
		fireEvent.click(removeButton("Loud Board"));
		const confirm = within(group("Loud Board")).getByRole("button", {
			name: "Remove",
		});
		fireEvent.click(confirm);
		fireEvent.click(confirm);
		expect(mocked.removePack).toHaveBeenCalledTimes(1);
		await act(async () => pending.resolve(removed([...PACKS, MINE], null)));
	});
});

describe("AC8: a removal that fails", () => {
	it("AC8: the row stays, the error names the pack and the reason, and it can be dismissed", async () => {
		await withImported("red");
		mocked.removePack.mockRejectedValue("Access is denied.");
		fireEvent.click(removeButton("My clicks"));
		fireEvent.click(
			within(group("My clicks")).getByRole("button", { name: "Remove" }),
		);

		const notice = await screen.findByRole("alert");
		expect(notice).toHaveTextContent(
			"Could not remove My clicks: Access is denied.",
		);
		expect(radio(/My clicks/)).toBeInTheDocument();
		expect(removeButton("My clicks")).toBeInTheDocument();
		expect(
			screen.queryByRole("group", { name: "Remove My clicks?" }),
		).not.toBeInTheDocument();
		expect(radio(/Red switches/)).toBeChecked();

		fireEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
		expect(screen.queryByText(/Could not remove/)).not.toBeInTheDocument();
	});
});

describe("AC9: a change that could not be saved", () => {
	it("AC9: an import whose change was not saved shows the save-failed notice", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(
			imported({}, { saved: false, save_error: "disk is full" }),
		);
		await loaded();
		fireEvent.click(importButton());
		const notice = await screen.findByText(/Could not save your settings/);
		expect(notice).toHaveTextContent("disk is full");
		expect(radio(/My clicks/)).toBeChecked();
	});

	it("AC9: a removal whose change was not saved shows the save-failed notice", async () => {
		await withImported("my-clicks");
		mocked.removePack.mockResolvedValue(
			removed(
				[...PACKS, LOUD],
				result(
					state(),
					{ pack_id: "brown" },
					{ saved: false, save_error: "read-only" },
				),
			),
		);
		fireEvent.click(removeButton("My clicks"));
		fireEvent.click(
			within(group("My clicks")).getByRole("button", { name: "Remove" }),
		);
		const notice = await screen.findByText(/Could not save your settings/);
		expect(notice).toHaveTextContent("read-only");
		expect(radio(/Brown switches/)).toBeChecked();
	});

	it("AC9: a saved change shows no such notice", async () => {
		healthyCore();
		mocked.importPack.mockResolvedValue(imported());
		await loaded();
		fireEvent.click(importButton());
		await waitFor(() => expect(radio(/My clicks/)).toBeChecked());
		expect(
			screen.queryByText(/Could not save your settings/),
		).not.toBeInTheDocument();
	});
});

describe("AC10: keyboard and focus", () => {
	it("AC10: the pack list is one radio group of native radios sharing one name", async () => {
		await withImported();
		expect(screen.getAllByRole("radiogroup")).toHaveLength(1);
		const radios = screen.getAllByRole("radio") as HTMLInputElement[];
		expect(radios).toHaveLength(6);
		expect(new Set(radios.map((r) => r.name)).size).toBe(1);
		expect(radios.every((r) => r.type === "radio")).toBe(true);
	});

	it("AC10: Import pack…, Preview and each Remove are buttons in the tab order, in visual order", async () => {
		await withImported();
		const mine = removeButton("My clicks");
		const loud = removeButton("Loud Board");
		for (const button of [importButton(), previewButton(), mine, loud]) {
			expect(button.tagName).toBe("BUTTON");
			expect(button.tabIndex).toBeGreaterThanOrEqual(0);
			expect(button).toBeEnabled();
		}
		const follows = (a: Element, b: Element) =>
			Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
		expect(follows(importButton(), previewButton())).toBe(true);
		expect(follows(previewButton(), radio(/My clicks/))).toBe(true);
		expect(follows(radio(/My clicks/), mine)).toBe(true);
		expect(follows(mine, radio(/Loud Board/))).toBe(true);
		expect(follows(radio(/Loud Board/), loud)).toBe(true);
	});

	it("AC10: no Remove button is inside a radio's label", async () => {
		await withImported();
		for (const button of screen.getAllByRole("button", { name: /^Remove / })) {
			expect(button.closest("label")).toBeNull();
		}
	});

	it("AC10: a Remove button is not a radio and pressing it does not choose the pack", async () => {
		await withImported("red");
		fireEvent.click(removeButton("My clicks"));
		expect(mocked.setSettings).not.toHaveBeenCalled();
		expect(radio(/Red switches/)).toBeChecked();
	});

	it("AC10: choosing an imported pack in the list works as for a bundled one", async () => {
		await withImported("red");
		fireEvent.click(radio(/Loud Board/));
		await waitFor(() =>
			expect(mocked.setSettings).toHaveBeenCalledWith({ pack_id: "loud" }),
		);
		await waitFor(() => expect(radio(/Loud Board/)).toBeChecked());
	});

	it("AC10: Import pack… keeps its focus while busy", async () => {
		healthyCore();
		const pending = deferred<ImportResult | null>();
		mocked.importPack.mockReturnValueOnce(pending.promise);
		await loaded();
		importButton().focus();
		fireEvent.click(importButton());
		await waitFor(() =>
			expect(importButton()).toHaveAttribute("aria-busy", "true"),
		);
		expect(importButton()).toHaveFocus();
		expect(importButton()).toHaveAttribute("aria-disabled", "true");
		await act(async () => pending.resolve(null));
	});
});
