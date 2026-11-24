// @vitest-environment node
// Feature 12: the version check (AC1 to AC3 and the plan's edge cases).
// The script runs as a child process against small fixture projects.
import { afterEach, describe, expect, it } from "vitest";
import { cleanupFixtures, makeProject, runCheck } from "./helpers.mjs";

afterEach(cleanupFixtures);

describe("AC1 equal and valid versions", () => {
	it("AC1 succeeds with no tag and prints the version alone", () => {
		const result = runCheck(makeProject({ version: "0.1.0" }));
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	it("AC1 fails when package.json differs and names each file with its value", () => {
		const result = runCheck(makeProject({ version: "0.1.0", pkg: "0.2.0" }));
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("package.json");
		expect(result.stderr).toContain("0.2.0");
		expect(result.stderr).toContain("package-lock.json");
		expect(result.stderr).toContain("Cargo.toml");
		expect(result.stderr).toContain("0.1.0");
	});

	it("AC1 fails when Cargo.toml differs and names it", () => {
		const result = runCheck(
			makeProject({ version: "0.1.0", cargoVersion: "0.3.0" }),
		);
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("src-tauri/Cargo.toml");
		expect(result.stderr).toContain("0.3.0");
	});

	it("AC1 fails when package-lock.json differs and names it", () => {
		const result = runCheck(
			makeProject({ version: "0.1.0", lock: "0.4.0", lockRoot: "0.4.0" }),
		);
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("package-lock.json");
		expect(result.stderr).toContain("0.4.0");
	});

	it("AC1 fails when the two version fields inside package-lock.json differ", () => {
		const result = runCheck(
			makeProject({ version: "0.1.0", lock: "0.1.0", lockRoot: "0.1.1" }),
		);
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("package-lock.json");
	});

	it("AC1 ignores a tauri.conf.json without a version", () => {
		const result = runCheck(
			makeProject({ version: "0.1.0", tauriConf: { productName: "X" } }),
		);
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	it("AC1 holds a version put back in tauri.conf.json equal: matching passes", () => {
		const result = runCheck(makeProject({ version: "0.1.0", tauri: "0.1.0" }));
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	it("AC1 fails when a version put back in tauri.conf.json differs and names it", () => {
		const result = runCheck(makeProject({ version: "0.1.0", tauri: "0.9.0" }));
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("tauri.conf.json");
		expect(result.stderr).toContain("0.9.0");
	});
});

describe("AC2 invalid versions", () => {
	for (const bad of ["", "1.0", "v1.0.0"]) {
		it(`AC2 fails for package.json version ${JSON.stringify(bad)} and says which file`, () => {
			const result = runCheck(makeProject({ version: "1.0.0", pkg: bad }));
			expect(result.code).toBe(1);
			expect(result.stderr).toContain("package.json");
			expect(result.stderr).toContain("is not a version like 1.2.3");
		});
	}

	it("AC2 fails for an invalid Cargo.toml version and says which file", () => {
		const result = runCheck(
			makeProject({
				cargo: '[package]\nname = "d"\nversion = "1.0"\n',
				version: "1.0.0",
			}),
		);
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("src-tauri/Cargo.toml");
	});

	it("AC2 fails for an invalid package-lock.json version and says which file", () => {
		const result = runCheck(
			makeProject({ version: "1.0.0", lock: "v1.0.0", lockRoot: "v1.0.0" }),
		);
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("package-lock.json");
	});

	it("AC2 fails for an invalid version in tauri.conf.json and says which file", () => {
		const result = runCheck(makeProject({ version: "1.0.0", tauri: "1.0" }));
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("tauri.conf.json");
	});

	it("AC2 accepts a pre-release part", () => {
		const result = runCheck(makeProject({ version: "0.2.0-beta.1" }));
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.2.0-beta.1");
	});
});

describe("AC3 the tag", () => {
	const root = () => makeProject({ version: "0.1.0" });

	it("AC3 succeeds for v0.1.0", () => {
		const result = runCheck(root(), "v0.1.0");
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	for (const tag of ["v0.1.1", "0.1.0", "v0.1.0-rc.1", "v0.1"]) {
		it(`AC3 fails for ${tag} and names the tag and the version`, () => {
			const result = runCheck(root(), tag);
			expect(result.code).toBe(1);
			expect(result.stderr).toContain(tag);
			expect(result.stderr).toContain("0.1.0");
		});
	}

	it("AC3 accepts a pre-release version with its matching tag", () => {
		const result = runCheck(
			makeProject({ version: "0.2.0-beta.1" }),
			"v0.2.0-beta.1",
		);
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.2.0-beta.1");
	});

	it("AC3 fails for a tag with a trailing newline", () => {
		const result = runCheck(root(), "v0.1.0\n");
		expect(result.code).toBe(1);
	});

	it("AC3 fails for a tag with the refs/tags/ prefix", () => {
		const result = runCheck(root(), "refs/tags/v0.1.0");
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("refs/tags/v0.1.0");
	});

	it("AC3 still fails on mismatched fields when the tag is right", () => {
		const result = runCheck(
			makeProject({ version: "0.1.0", pkg: "0.1.1" }),
			"v0.1.0",
		);
		expect(result.code).toBe(1);
	});
});

describe("Cargo.toml reading (plan edge cases)", () => {
	it("a version in a dependency table before [package] is not the package version", () => {
		const cargo =
			'[dependencies.foo]\nversion = "9.9.9"\n\n[package]\nname = "d"\nversion = "0.1.0"\n';
		const result = runCheck(makeProject({ version: "0.1.0", cargo }));
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	it("a version in a dependency table after [package] is not the package version", () => {
		const cargo =
			'[package]\nname = "d"\nversion = "0.1.0"\n\n[dependencies.foo]\nversion = "9.9.9"\n';
		const result = runCheck(makeProject({ version: "0.1.0", cargo }));
		expect(result.code).toBe(0);
		expect(result.stdout.trim()).toBe("0.1.0");
	});

	it("version = { workspace = true } is not a version and names the file", () => {
		const cargo = '[package]\nname = "d"\nversion = { workspace = true }\n';
		const result = runCheck(makeProject({ version: "0.1.0", cargo }));
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("src-tauri/Cargo.toml");
		expect(result.stderr).toContain("is not a version like 1.2.3");
	});

	it("a [package] table without a version fails and names the file", () => {
		const cargo =
			'[package]\nname = "d"\n\n[dependencies.foo]\nversion = "0.1.0"\n';
		const result = runCheck(makeProject({ version: "0.1.0", cargo }));
		expect(result.code).toBe(1);
		expect(result.stderr).toContain("src-tauri/Cargo.toml");
	});
});

describe("missing files", () => {
	for (const name of [
		"package.json",
		"package-lock.json",
		"src-tauri/Cargo.toml",
	]) {
		it(`fails with the path of a missing ${name}, without a stack trace`, () => {
			const result = runCheck(makeProject({ version: "0.1.0", omit: [name] }));
			expect(result.code).toBe(1);
			expect(result.stderr).toContain(name);
		});
	}
});
