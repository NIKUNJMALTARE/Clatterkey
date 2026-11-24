// @vitest-environment node
// Feature 12 AC5 (as changed by feature 14 AC19) and feature 14 AC20: the
// bundle configuration and the package names, read from the real files.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { repoRoot } from "./helpers.mjs";

const conf = JSON.parse(
	readFileSync(join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
);
const bundle = conf.bundle;
const targets = Array.isArray(bundle.targets)
	? bundle.targets
	: [bundle.targets];

describe("AC5 bundle configuration", () => {
	it("AC5 signs the macOS app with the ad-hoc identity", () => {
		expect(bundle.macOS.signingIdentity).toBe("-");
	});

	it("AC5 sets the macOS minimum system version to 12.0", () => {
		expect(bundle.macOS.minimumSystemVersion).toBe("12.0");
	});

	it("AC5 builds the disk image and the NSIS installer, and no msi", () => {
		expect(targets).toContain("dmg");
		expect(targets).toContain("nsis");
		expect(targets).not.toContain("msi");
		expect(targets).not.toContain("all");
	});

	it("AC5 makes the Windows installer a per-user install", () => {
		expect(bundle.windows.nsis.installMode).toBe("currentUser");
	});

	it("AC5 still includes the bundled packs as resources", () => {
		expect(bundle.resources).toContain("resources/packs/");
	});

	it("AC5 keeps the uninstall hook", () => {
		expect(bundle.windows.nsis.installerHooks).toBe("./windows/hooks.nsh");
	});
});

describe("AC19 the name and identifier", () => {
	it("AC19 the product name is Clatterkey", () => {
		expect(conf.productName).toBe("Clatterkey");
	});

	it("AC19 the identifier is com.clatterkey.app", () => {
		expect(conf.identifier).toBe("com.clatterkey.app");
	});

	it("AC19 the window title is Clatterkey", () => {
		expect(conf.app.windows[0].title).toBe("Clatterkey");
	});

	it("AC19 the document title in index.html is Clatterkey (AC1)", () => {
		const html = readFileSync(join(repoRoot, "index.html"), "utf8");
		expect(html).toMatch(/<title>Clatterkey<\/title>/);
	});
});

describe("AC20 the package names", () => {
	const cargo = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");
	const pkg = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
	const lock = JSON.parse(
		readFileSync(join(repoRoot, "package-lock.json"), "utf8"),
	);
	// The first value of `key = "..."` inside the [section] of a TOML text.
	function tomlValue(section, key) {
		const start = cargo.indexOf(`[${section}]`);
		if (start === -1) return undefined;
		const rest = cargo.slice(start + section.length + 2);
		const next = rest.search(/^\[/m);
		const body = next === -1 ? rest : rest.slice(0, next);
		return new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m").exec(body)?.[1];
	}

	it("AC20 the Cargo package is clatterkey", () => {
		expect(tomlValue("package", "name")).toBe("clatterkey");
	});

	it("AC20 the library is clatterkey_lib", () => {
		expect(tomlValue("lib", "name")).toBe("clatterkey_lib");
	});

	it("AC20 the npm package is clatterkey, in both lock file name fields", () => {
		expect(pkg.name).toBe("clatterkey");
		expect(lock.name).toBe("clatterkey");
		expect(lock.packages[""].name).toBe("clatterkey");
	});

	it("AC20 main.rs starts the renamed library", () => {
		const main = readFileSync(join(repoRoot, "src-tauri/src/main.rs"), "utf8");
		expect(main).toContain("clatterkey_lib::run()");
	});

	it("AC20 the version stays 0.1.0 everywhere", () => {
		expect(tomlValue("package", "version")).toBe("0.1.0");
		expect(pkg.version).toBe("0.1.0");
		expect(conf.version ?? "0.1.0").toBe("0.1.0");
	});
});
