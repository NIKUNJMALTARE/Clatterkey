// Shared by the feature 12 tests: the repository root, a fixture project
// builder and a runner for scripts/check-version.mjs.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repoRoot = resolve(
	dirname(fileURLToPath(import.meta.url)),
	"../..",
);

const fixtures = [];

export function cleanupFixtures() {
	for (const dir of fixtures.splice(0)) {
		rmSync(dir, { recursive: true, force: true });
	}
}

function cargoToml(version) {
	return `[package]\nname = "demo"\nversion = ${version}\nedition = "2024"\n`;
}

// Builds a small project folder. `version` sets every file at once; the
// per-file options override it. `omit` lists files to leave out, `cargo` is a
// full Cargo.toml text and `tauriConf` a full tauri.conf.json object.
export function makeProject({
	version = "0.1.0",
	pkg = version,
	lock = version,
	lockRoot = lock,
	cargoVersion = version,
	cargo,
	tauri = null,
	tauriConf = null,
	omit = [],
} = {}) {
	const root = mkdtempSync(join(tmpdir(), "check-version-"));
	fixtures.push(root);
	mkdirSync(join(root, "src-tauri"), { recursive: true });
	const write = (name, content) => {
		if (!omit.includes(name)) writeFileSync(join(root, name), content);
	};
	write("package.json", JSON.stringify({ name: "demo", version: pkg }));
	write(
		"package-lock.json",
		JSON.stringify({
			name: "demo",
			version: lock,
			lockfileVersion: 3,
			packages: { "": { name: "demo", version: lockRoot } },
		}),
	);
	write(
		"src-tauri/Cargo.toml",
		cargo ?? cargoToml(JSON.stringify(cargoVersion)),
	);
	if (tauriConf !== null) {
		write("src-tauri/tauri.conf.json", JSON.stringify(tauriConf));
	} else if (tauri !== null) {
		write("src-tauri/tauri.conf.json", JSON.stringify({ version: tauri }));
	}
	return root;
}

export function runCheck(root, tag) {
	const args = ["scripts/check-version.mjs", "--root", root];
	if (tag !== undefined) args.push("--tag", tag);
	const result = spawnSync("node", args, {
		cwd: repoRoot,
		encoding: "utf8",
	});
	const output = {
		code: result.status,
		stdout: result.stdout,
		stderr: result.stderr,
	};
	// A missing script or a crash is not a refusal: fail the test loudly.
	if (/Cannot find module|ERR_MODULE_NOT_FOUND|\n\s+at /.test(output.stderr)) {
		throw new Error(`check-version.mjs did not run cleanly:\n${output.stderr}`);
	}
	return output;
}
