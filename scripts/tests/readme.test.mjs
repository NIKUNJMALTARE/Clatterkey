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
