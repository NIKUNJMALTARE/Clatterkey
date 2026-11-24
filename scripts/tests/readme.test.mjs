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

// ---- Feature 14 AC23: the new name, and the section about the earlier ones ----

// The earlier names and identifiers, spelled from parts so that this file
// itself holds no occurrence of them (see old-name.test.mjs).
const CLICK = ["Click", "Clak"].join("");
const MKS = ["Mechanical", "Key", "Sounds"].join(" ");
const CLICK_ID = ["com", CLICK.toLowerCase(), "app"].join(".");
const MKS_ID = ["com", ["mechanical", "key", "sounds"].join(""), "app"].join(
	".",
);
const NEW_ID = "com.clatterkey.app";
const MOVE = section("coming from an earlier name");

describe("AC23 README under the new name", () => {
	it("AC23 is titled Clatterkey", () => {
		expect(lines[0]).toBe("# Clatterkey");
	});

	it("AC23 gives the new installer names", () => {
		expect(readme).toContain("Clatterkey_<version>_aarch64.dmg");
		expect(readme).toContain("Clatterkey_<version>_x64-setup.exe");
	});

	it("AC23 gives the new settings folders for both systems", () => {
		expect(readme).toContain(
			`~/Library/Application Support/${NEW_ID}/settings.json`,
		);
		expect(readme).toContain(`%APPDATA%\\${NEW_ID}\\settings.json`);
	});

	it("AC23 gives the launch agent file name Clatterkey.plist", () => {
		expect(readme).toContain("Clatterkey.plist");
	});

	it("AC23 gives the tccutil identifier", () => {
		expect(readme).toContain(`tccutil reset ListenEvent ${NEW_ID}`);
	});

	it("AC23 titles the release Clatterkey X.Y.Z", () => {
		expect(readme).toContain('"Clatterkey X.Y.Z"');
	});

	it("AC23 says Quit Clatterkey for the tray item", () => {
		expect(readme).toContain("**Quit Clatterkey**");
	});

	it("AC23 has one section about earlier names, and the old heading is gone", () => {
		expect(MOVE).toBeDefined();
		expect(readme).not.toMatch(new RegExp(`^#+\\s+Coming from ${MKS}`, "mi"));
		const headings = lines.filter(
			(l) => headingLevel(l) > 0 && /coming from/i.test(l),
		);
		expect(headings).toHaveLength(1);
	});

	it("AC23 names the earlier names only inside that section", () => {
		const outside = MOVE === undefined ? readme : readme.replace(MOVE, "");
		for (const spelling of [
			new RegExp(["mechanical", "key", "sounds"].join("[ _-]"), "i"),
			new RegExp(["mechanical", "key", "sounds"].join(""), "i"),
			new RegExp(CLICK, "i"),
		]) {
			expect(outside).not.toMatch(spelling);
		}
	});
});

describe('AC23 the section "Coming from an earlier name"', () => {
	const move = MOVE ?? "";

	it("AC23 exists", () => {
		expect(MOVE).toBeDefined();
	});

	it("AC23 names both earlier names and both of their identifiers", () => {
		expect(move).toContain(CLICK);
		expect(move).toContain(MKS);
		expect(move).toContain(CLICK_ID);
		expect(move).toContain(MKS_ID);
	});

	it("AC23 says settings and packs are carried over at the first start, from the newer name first", () => {
		expect(move).toMatch(/settings/i);
		expect(move).toMatch(/packs/i);
		expect(move).toMatch(/carried over|copied/i);
		expect(move).toMatch(/first (start|launch|time)/i);
		expect(move).toMatch(/otherwise/i);
		expect(move.indexOf(CLICK)).toBeLessThan(move.indexOf(MKS));
	});

	it("AC23 says the macOS permission is asked again", () => {
		expect(move).toMatch(/macOS/);
		expect(move).toMatch(/Input Monitoring|permission/);
		expect(move).toMatch(/again/i);
	});

	it("AC23 says to remove the earlier app, and how on each system", () => {
		expect(move).toMatch(/remove|uninstall/i);
		expect(move).toContain(`${CLICK}.app`);
		expect(move).toContain(`${MKS}.app`);
		expect(move).toMatch(/Bin/);
		expect(move).toContain("Installed apps");
	});

	it("AC23 says the earlier folders can be deleted afterwards", () => {
		expect(move).toMatch(/delete/i);
	});

	it("AC23 says how to copy a folder by hand, with a row per earlier folder and system", () => {
		expect(move).toMatch(/by hand|manually|copy/i);
		expect(move).toContain("settings.json");
		for (const id of [CLICK_ID, MKS_ID]) {
			expect(move).toContain(`~/Library/Application Support/${id}`);
			expect(move).toContain(`%APPDATA%\\${id}`);
		}
		expect(move).toContain(`~/Library/Application Support/${NEW_ID}`);
		expect(move).toContain(`%APPDATA%\\${NEW_ID}`);
	});

	it("AC23 warns that both apps at once sound every key twice", () => {
		expect(move).toMatch(/twice/i);
	});
});
