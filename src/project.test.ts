import { describe, expect, it } from "vitest";
import cargoToml from "../src-tauri/Cargo.toml?raw";
import packageJsonText from "../package.json?raw";
import toolchainToml from "../rust-toolchain.toml?raw";

// Globs so that a missing file is a failing assertion, not an import crash.
const optional = (files: Record<string, unknown>) =>
	Object.values(files)[0] as string | undefined;

const workflow = optional(
	import.meta.glob("../.github/workflows/check.yml", {
		query: "?raw",
		import: "default",
		eager: true,
	}),
);
const npmLock = optional(
	import.meta.glob("../package-lock.json", {
		query: "?raw",
		import: "default",
		eager: true,
	}),
);
const cargoLock = optional(
	import.meta.glob("../src-tauri/Cargo.lock", {
		query: "?raw",
		import: "default",
		eager: true,
	}),
);

const pkg = JSON.parse(packageJsonText) as {
	dependencies: Record<string, string>;
	devDependencies: Record<string, string>;
};
const deps = { ...pkg.dependencies, ...pkg.devDependencies };

/** Major.minor a semver range such as "~19.3.1" or "^8.0.0" starts at. */
const majorMinor = (range: string) => {
	const m = range.match(/(\d+)\.(\d+)/);
	return m ? `${m[1]}.${m[2]}` : range;
};

describe("AC8: stack versions", () => {
	it.each([
		["react", "19.3"],
		["react-dom", "19.3"],
		["typescript", "7.0"],
		["vite", "8.3"],
		["vitest", "5.1"],
		["@biomejs/biome", "2.6"],
		["@tauri-apps/cli", "2.12"],
	])("AC8: %s is at %s", (name, expected) => {
		expect(deps[name], `${name} missing from package.json`).toBeDefined();
		expect(majorMinor(deps[name])).toBe(expected);
	});

	it("AC8: tauri crate is at 2.12", () => {
		const line = cargoToml.split("\n").find((l) => /^tauri\s*=/.test(l.trim()));
		expect(line, "tauri dependency missing from Cargo.toml").toBeDefined();
		const version = (line as string).match(/"(\d+\.\d+[^"]*)"/);
		expect(version?.[1] && majorMinor(version[1])).toBe("2.12");
	});

	it("AC8: Rust toolchain is 1.96", () => {
		const channel = toolchainToml.match(/channel\s*=\s*"([^"]+)"/);
		expect(channel?.[1] && majorMinor(channel[1])).toBe("1.96");
	});

	it("AC8: a lock file exists for the UI side", () => {
		expect(npmLock, "package-lock.json missing").toBeTruthy();
	});

	it("AC8: a lock file exists for the core side", () => {
		expect(cargoLock, "src-tauri/Cargo.lock missing").toBeTruthy();
	});
});

describe("AC9: .github/workflows/check.yml", () => {
	const text = () => {
		expect(workflow, ".github/workflows/check.yml missing").toBeTruthy();
		return workflow as string;
	};
	const uses = () =>
		[...text().matchAll(/^\s*(?:-\s*)?uses:\s*(\S+)/gm)].map((m) => m[1]);

	it("AC9: triggers on pull_request", () => {
		expect(text()).toMatch(/^on:[\s\S]*?\bpull_request\b/m);
	});

	it("AC9: runs on ubuntu-latest", () => {
		expect(text()).toMatch(/runs-on:\s*ubuntu-latest/);
	});

	it("AC9: pins every action to a major version", () => {
		const actions = uses();
		expect(actions.length).toBeGreaterThan(0);
		for (const action of actions) {
			expect(action, action).toMatch(/@v\d+$/);
		}
	});

	it("AC9: sets up Node", () => {
		expect(uses().some((a) => a.startsWith("actions/setup-node@"))).toBe(true);
	});

	it("AC9: sets up Rust", () => {
		// Real step content only: comment lines are ignored.
		const lines = text().split("\n");
		const commands: string[] = [];
		lines.forEach((line, i) => {
			const m = line.match(/^(\s*)(?:-\s*)?run:\s*(.*)$/);
			if (!m || m[2].trim().startsWith("#")) return;
			if (/^[|>][-+]?\s*$/.test(m[2])) {
				const indent = m[1].length;
				for (const next of lines.slice(i + 1)) {
					if (next.trim() && next.search(/\S/) <= indent) break;
					if (!next.trim().startsWith("#")) commands.push(next.trim());
				}
			} else {
				commands.push(m[2].trim());
			}
		});
		const runsRustup = commands.some((c) => /^rustup\b/.test(c));
		const usesRustAction = uses().some((a) =>
			a.startsWith("dtolnay/rust-toolchain@"),
		);
		expect(runsRustup || usesRustAction).toBe(true);
	});

	it("AC9: installs the Tauri Linux build packages", () => {
		const t = text();
		expect(t).toMatch(/apt-get install/);
		for (const pkgName of [
			"libwebkit2gtk-4.1-dev",
			"libxdo-dev",
			"libssl-dev",
			"libayatana-appindicator3-dev",
			"librsvg2-dev",
		]) {
			expect(t).toContain(pkgName);
		}
	});

	it("AC9: runs npm ci", () => {
		expect(text()).toMatch(/run:\s*npm ci\b/);
	});

	it("AC9: its last step is scripts/check.sh", () => {
		const runs = [...text().matchAll(/^\s*(?:-\s*)?run:\s*(.+)$/gm)];
		expect(runs.length).toBeGreaterThan(0);
		expect(runs[runs.length - 1][1].trim()).toMatch(/scripts\/check\.sh$/);
		const steps = text().split(/^\s*-\s+(?=name:|uses:|run:)/m);
		expect(steps[steps.length - 1]).toMatch(/scripts\/check\.sh/);
	});
});
