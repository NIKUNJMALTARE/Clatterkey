// @vitest-environment node
// Feature 12 AC5: the bundle configuration, read from the real file.
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

	it("AC5 keeps the identifier and the product name", () => {
		expect(conf.identifier).toBe("com.mechanicalkeysounds.app");
		expect(conf.productName).toBe("Mechanical Key Sounds");
	});
});
