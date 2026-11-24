// @vitest-environment node
// Feature 14 AC24: the repository, outside docs/features/ and the
// hand-over's own code, tests and README section, holds no occurrence of
// either earlier name in any of its spellings.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { repoRoot } from "./helpers.mjs";

// Spelled from parts so that this file does not match itself.
const OLD_NAME = ["Mechanical", "Key", "Sounds"].join(" ");
const needles = [
	new RegExp(OLD_NAME),
	new RegExp(["mechanical", "key", "sounds"].join("[ _-]"), "i"),
	new RegExp(["mechanical", "key", "sounds"].join(""), "i"),
	new RegExp(["click", "clak"].join(""), "i"),
];

// Allowed to name the old app: the hand-over module and its tests, and the
// README section about the move. Everything under docs/features/ is history.
const ALLOWED_FILES = new Set([
	"src-tauri/src/handover.rs",
	"src-tauri/tests/handover.rs",
]);
const ALLOWED_PREFIXES = ["docs/features/"];

const tracked = execFileSync("git", ["ls-files", "-z"], {
	cwd: repoRoot,
	encoding: "utf8",
	maxBuffer: 64 * 1024 * 1024,
})
	.split("\0")
	.filter(Boolean);

// The README without its "Coming from ..." section.
function withoutMoveSection(text) {
	const lines = text.split("\n");
	const level = (line) => /^(#{1,6})\s/.exec(line)?.[1].length ?? 0;
	const start = lines.findIndex(
		(line) =>
			level(line) > 0 && line.replace(/^#+\s+/, "").startsWith("Coming from"),
	);
	if (start === -1) return text;
	let end = lines.length;
	for (let i = start + 1; i < lines.length; i++) {
		if (level(lines[i]) > 0 && level(lines[i]) <= level(lines[start])) {
			end = i;
			break;
		}
	}
	return [...lines.slice(0, start), ...lines.slice(end)].join("\n");
}

describe("AC24 the old name is gone", () => {
	it("AC24 the search covers the repository's tracked files", () => {
		expect(tracked.length).toBeGreaterThan(50);
		expect(tracked).toContain("README.md");
		expect(tracked).toContain("src-tauri/tauri.conf.json");
	});

	it("AC24 no tracked file outside the allowed places names the old app", () => {
		const found = [];
		for (const file of tracked) {
			if (ALLOWED_FILES.has(file)) continue;
			if (ALLOWED_PREFIXES.some((prefix) => file.startsWith(prefix))) continue;
			let text;
			try {
				text = readFileSync(join(repoRoot, file));
			} catch {
				continue; // deleted in the working tree
			}
			if (text.includes(0)) continue; // binary
			let content = text.toString("utf8");
			if (file === "README.md") content = withoutMoveSection(content);
			for (const needle of needles) {
				if (needle.test(content)) found.push(`${file} (${needle})`);
			}
		}
		expect(found).toEqual([]);
	});

	it("AC24 the file names themselves are clean", () => {
		const found = tracked.filter(
			(file) =>
				!file.startsWith("docs/features/") && needles.some((n) => n.test(file)),
		);
		expect(found).toEqual([]);
	});
});
