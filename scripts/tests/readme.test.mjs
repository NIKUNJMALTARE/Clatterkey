// @vitest-environment node
// Feature 12 AC7: the README, read from the real file.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { repoRoot } from "./helpers.mjs";

const readme = readFileSync(join(repoRoot, "README.md"), "utf8");
const lines = readme.split("\n");

function headingLevel(line) {
	return /^(#{1,6})\s/.exec(line)?.[1].length ?? 0;
}

// The text of a section: from its heading to the next heading of the same
// or a higher level. Returns undefined when there is no such heading.
function section(name) {
	const start = lines.findIndex(
		(line) =>
			headingLevel(line) > 0 &&
			line
				.replace(/^#+\s+/, "")
				.trim()
				.toLowerCase() === name,
	);
	if (start === -1) return undefined;
	const level = headingLevel(lines[start]);
	let end = lines.length;
	for (let i = start + 1; i < lines.length; i++) {
		const l = headingLevel(lines[i]);
		if (l > 0 && l <= level) {
			end = i;
			break;
		}
	}
	return lines.slice(start, end).join("\n");
}

describe("AC7 README", () => {
	for (const name of ["install", "updating", "uninstalling", "releasing"]) {
		it(`AC7 has a ${name} section`, () => {
			expect(section(name)).toBeDefined();
		});
	}

	describe("the Install section", () => {
		const install = section("install") ?? "";

		for (const phrase of [
			"Open Anyway",
			"More info",
			"Run anyway",
			"Input Monitoring",
			"macOS 12",
			"Windows 10",
		]) {
			it(`AC7 names "${phrase}"`, () => {
				expect(install).toContain(phrase);
			});
		}

		it("AC7 links to the releases page", () => {
			expect(install).toMatch(/https:\/\/github\.com\/[^\s)]+\/releases/);
		});
	});

	it('AC7 no longer says "under construction"', () => {
		expect(readme).not.toMatch(/under construction/i);
	});
});

// ---- Feature 13 AC24: the new name, and the section about the old one ----

// The old name and its identifier, spelled from parts so that this file
// itself holds no occurrence of them (see old-name.test.mjs).
const OLD_NAME = ["Mechanical", "Key", "Sounds"].join(" ");
const OLD_ID = ["com", ["mechanical", "key", "sounds"].join(""), "app"].join(
	".",
);
const NEW_ID = "com.clickclak.app";
const MOVE = section(`coming from ${OLD_NAME}`.toLowerCase());

describe("AC24 README under the new name", () => {
	it("AC24 is titled ClickClak", () => {
		expect(lines[0]).toBe("# ClickClak");
	});

	it("AC24 gives the new installer names", () => {
		expect(readme).toContain("ClickClak_<version>_aarch64.dmg");
		expect(readme).toContain("ClickClak_<version>_x64-setup.exe");
	});

	it("AC24 gives the new settings folders for both systems", () => {
		expect(readme).toContain(
			`~/Library/Application Support/${NEW_ID}/settings.json`,
		);
		expect(readme).toContain(`%APPDATA%\\${NEW_ID}\\settings.json`);
	});

	it("AC24 gives the launch agent file name ClickClak.plist", () => {
		expect(readme).toContain("ClickClak.plist");
	});

	it("AC24 gives the tccutil identifier", () => {
		expect(readme).toContain(`tccutil reset ListenEvent ${NEW_ID}`);
	});

	it("AC24 titles the release ClickClak X.Y.Z", () => {
		expect(readme).toContain('"ClickClak X.Y.Z"');
	});

	it("AC24 says Quit ClickClak for the tray item", () => {
		expect(readme).toContain("**Quit ClickClak**");
	});

	it("AC24 has no hyphenated old installer name", () => {
		expect(readme).not.toMatch(
			new RegExp(["Mechanical", "Key", "Sounds"].join("-"), "i"),
		);
	});

	it("AC24 names the old name only inside the section about it", () => {
		const outside = MOVE === undefined ? readme : readme.replace(MOVE, "");
		for (const spelling of [
			new RegExp(["mechanical", "key", "sounds"].join("[ _-]"), "i"),
			new RegExp(["mechanical", "key", "sounds"].join(""), "i"),
		]) {
			expect(outside).not.toMatch(spelling);
		}
	});
});

describe(`AC24 the section "Coming from ${OLD_NAME}"`, () => {
	const move = MOVE ?? "";

	it("AC24 exists", () => {
		expect(MOVE).toBeDefined();
	});

	it("AC24 says settings and packs are carried over at the first start", () => {
		expect(move).toMatch(/settings/i);
		expect(move).toMatch(/packs/i);
		expect(move).toMatch(/carried over|copied/i);
		expect(move).toMatch(/first (start|launch|time)/i);
	});

	it("AC24 says the macOS permission is asked again", () => {
		expect(move).toMatch(/macOS/);
		expect(move).toMatch(/Input Monitoring|permission/);
		expect(move).toMatch(/again/i);
	});

	it("AC24 says to remove the old app, and how on each system", () => {
		expect(move).toMatch(/remove|uninstall/i);
		expect(move).toContain(`${OLD_NAME}.app`);
		expect(move).toMatch(/Bin/);
		expect(move).toContain("Installed apps");
	});

	it("AC24 says the old folder can be deleted afterwards", () => {
		expect(move).toMatch(/delete/i);
		expect(move).toContain(OLD_ID);
	});

	it("AC24 says how to copy the folder by hand, with both folders per system", () => {
		expect(move).toMatch(/by hand|manually|copy/i);
		expect(move).toContain("settings.json");
		expect(move).toContain(`~/Library/Application Support/${OLD_ID}`);
		expect(move).toContain(`~/Library/Application Support/${NEW_ID}`);
		expect(move).toContain(`%APPDATA%\\${OLD_ID}`);
		expect(move).toContain(`%APPDATA%\\${NEW_ID}`);
	});

	it("AC24 warns that both apps at once sound every key twice", () => {
		expect(move).toMatch(/twice/i);
	});
});
