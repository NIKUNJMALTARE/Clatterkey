// @vitest-environment node
// Feature 12 AC4: scripts/check.sh runs the version check, and the check
// passes on the repository as committed.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { repoRoot } from "./helpers.mjs";

const checkSh = readFileSync(join(repoRoot, "scripts/check.sh"), "utf8");

describe("AC4 scripts/check.sh", () => {
	it("AC4 runs the version check through step, so a failure exits non-zero", () => {
		expect(checkSh).toMatch(
			/^step\s+"[^"]*"\s+node scripts\/check-version\.mjs\s*$/m,
		);
		expect(checkSh).not.toMatch(/check-version\.mjs[^\n]*(\|\||;\s*true)/);
	});

	it("AC4 runs the version check before the format steps", () => {
		const version = checkSh.indexOf("check-version.mjs");
		const format = checkSh.indexOf("biome format");
		expect(version).toBeGreaterThan(-1);
		expect(version).toBeLessThan(format);
	});

	it("AC4 passes on the repository as committed", () => {
		const result = spawnSync("node", ["scripts/check-version.mjs"], {
			cwd: repoRoot,
			encoding: "utf8",
		});
		expect(result.stderr).toBe("");
		expect(result.status).toBe(0);
		expect(result.stdout.trim()).toMatch(
			/^\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$/,
		);
	});
});
