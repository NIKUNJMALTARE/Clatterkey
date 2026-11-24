// @vitest-environment node
// Feature 12 AC6: the release workflow and check.yml, parsed from the real
// files.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";
import { repoRoot } from "./helpers.mjs";

const read = (path) => readFileSync(join(repoRoot, path), "utf8");
const releaseText = read(".github/workflows/release.yml");
const release = parse(releaseText);
const check = parse(read(".github/workflows/check.yml"));

const allowedPaths = [
	".github/workflows/release.yml",
	"src-tauri/tauri.conf.json",
	"src-tauri/windows/**",
	"scripts/check-version.mjs",
];

const asList = (value) => (Array.isArray(value) ? value : [value]);

// The write scopes a `permissions` value grants, as "scope: level" strings.
function writes(permissions) {
	if (permissions === undefined || permissions === null) return [];
	if (typeof permissions === "string") {
		return permissions === "write-all" ? ["write-all"] : [];
	}
	return Object.entries(permissions)
		.filter(([, level]) => level === "write")
		.map(([scope]) => scope);
}

describe("AC6 release workflow", () => {
	it("AC6 has the jobs version, build, sums and release", () => {
		for (const job of ["version", "build", "sums", "release"]) {
			expect(release.jobs).toHaveProperty(job);
		}
	});

	it("AC6 starts on a pushed tag matching v*", () => {
		expect(release.on.push.tags).toContain("v*");
	});

	it("AC6 starts on a manual run", () => {
		expect(Object.keys(release.on)).toContain("workflow_dispatch");
	});

	it("AC6 starts on pull requests limited by path to exactly the release files", () => {
		const paths = release.on.pull_request?.paths;
		expect(Array.isArray(paths)).toBe(true);
		expect([...paths].sort()).toEqual([...allowedPaths].sort());
	});

	it("AC6 grants no write permission at the top level or on any job but release", () => {
		expect(writes(release.permissions)).toEqual([]);
		for (const [name, job] of Object.entries(release.jobs)) {
			if (name === "release") continue;
			expect(writes(job.permissions), `job ${name}`).toEqual([]);
		}
	});

	it("AC6 gives the release job contents: write and nothing else", () => {
		expect(release.jobs.release.permissions.contents).toBe("write");
		expect(writes(release.jobs.release.permissions)).toEqual(["contents"]);
	});

	it("AC6 runs the release job only for a pushed tag", () => {
		const condition = String(release.jobs.release.if ?? "");
		expect(condition).toMatch(/github\.event_name\s*==\s*'push'/);
		expect(condition).toMatch(/refs\/tags\/|ref_type\s*==\s*'tag'/);
	});

	it("AC6 makes the release job need the build jobs", () => {
		expect(asList(release.jobs.release.needs)).toContain("build");
	});

	it("AC6 builds on a macOS and a Windows runner", () => {
		const include = release.jobs.build.strategy?.matrix?.include ?? [];
		const runners = include.flatMap((leg) => Object.values(leg));
		expect(runners).toContain("macos-latest");
		expect(runners).toContain("windows-latest");
	});

	it("AC6 uses no secret other than the GitHub token", () => {
		const used = [...releaseText.matchAll(/secrets\.([A-Za-z0-9_]+)/g)].map(
			(m) => m[1],
		);
		for (const name of used) expect(name).toBe("GITHUB_TOKEN");
		expect(releaseText).not.toMatch(/secrets:\s*inherit/);
	});
});

describe("AC6 check.yml is unchanged in what it checks", () => {
	it("AC6 still runs on every pull request, with no path or branch filter", () => {
		expect(Object.keys(check.on)).toContain("pull_request");
		const pr = check.on.pull_request;
		if (pr !== null && typeof pr === "object") {
			for (const key of [
				"paths",
				"paths-ignore",
				"branches",
				"branches-ignore",
			]) {
				expect(pr).not.toHaveProperty(key);
			}
		}
	});

	it("AC6 still runs scripts/check.sh", () => {
		const runs = Object.values(check.jobs)
			.flatMap((job) => job.steps)
			.map((step) => step.run ?? "");
		expect(runs.some((run) => run.includes("scripts/check.sh"))).toBe(true);
	});
});
