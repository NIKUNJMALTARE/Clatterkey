#!/usr/bin/env node
// Checks that every file holding the project's version agrees, and, given a
// tag, that the tag is `v` followed by that version.
//
//   node scripts/check-version.mjs [--root <dir>] [--tag <tag>]
//
// Success: exit 0, the version alone on standard output.
// Failure: exit 1, the reason on standard error.
//
// Used by scripts/check.sh and by .github/workflows/release.yml.
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = /^\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$/;

const PACKAGE_JSON = "package.json";
const PACKAGE_LOCK = "package-lock.json";
const CARGO_TOML = "src-tauri/Cargo.toml";
const TAURI_CONF = "src-tauri/tauri.conf.json";

function fail(lines) {
	for (const line of lines) console.error(line);
	process.exit(1);
}

function parseArgs(argv) {
	const options = {
		root: resolve(dirname(fileURLToPath(import.meta.url)), ".."),
		tag: undefined,
	};
	for (let i = 0; i < argv.length; i++) {
		const name = argv[i];
		if (name !== "--root" && name !== "--tag") {
			fail([
				`unknown argument: ${name}`,
				"usage: node scripts/check-version.mjs [--root <dir>] [--tag <tag>]",
			]);
		}
		const value = argv[++i];
		if (value === undefined) fail([`${name} needs a value`]);
		if (name === "--root") options.root = resolve(value);
		else options.tag = value;
	}
	return options;
}

// How a value is shown in a message.
function show(value) {
	return value === undefined ? "(missing)" : JSON.stringify(value);
}

// The `version` of the `[package]` table. A value that is not a plain string
// (`{ workspace = true }`) is returned as written, so it is reported as not
// a version.
function cargoPackageVersion(text) {
	let inPackage = false;
	for (const raw of text.split(/\r?\n/)) {
		const line = raw.trim();
		if (line.startsWith("[")) {
			inPackage = line === "[package]";
			continue;
		}
		if (!inPackage) continue;
		const match = /^version\s*=\s*(.*)$/.exec(line);
		if (!match) continue;
		const value = match[1].replace(/\s+#.*$/, "").trim();
		const quoted = /^"([^"]*)"$/.exec(value) ?? /^'([^']*)'$/.exec(value);
		return quoted ? quoted[1] : value;
	}
	return undefined;
}

const { root, tag } = parseArgs(process.argv.slice(2));
const problems = [];
// One entry per file that holds a version: { path, value }.
const found = [];

function read(path) {
	try {
		return readFileSync(join(root, path), "utf8");
	} catch (error) {
		problems.push(
			`${path}: could not be read (${error.code ?? error.message})`,
		);
		return undefined;
	}
}

function readJson(path) {
	const text = read(path);
	if (text === undefined) return undefined;
	try {
		return JSON.parse(text);
	} catch (error) {
		problems.push(`${path}: is not valid JSON (${error.message})`);
		return undefined;
	}
}

const pkg = readJson(PACKAGE_JSON);
if (pkg !== undefined) found.push({ path: PACKAGE_JSON, value: pkg?.version });

const lock = readJson(PACKAGE_LOCK);
if (lock !== undefined) {
	const top = lock?.version;
	const inner = lock?.packages?.[""]?.version;
	if (top !== inner) {
		problems.push(
			`${PACKAGE_LOCK}: its two version fields differ: version is ${show(top)}, packages[""].version is ${show(inner)}`,
		);
	} else {
		found.push({ path: PACKAGE_LOCK, value: top });
	}
}

const cargo = read(CARGO_TOML);
if (cargo !== undefined) {
	found.push({ path: CARGO_TOML, value: cargoPackageVersion(cargo) });
}

// tauri.conf.json normally has no version (Tauri takes Cargo.toml's). One put
// back by mistake is held equal to the rest. A missing file is not an error
// here: only the field matters.
let tauriConf;
try {
	tauriConf = JSON.parse(readFileSync(join(root, TAURI_CONF), "utf8"));
} catch (error) {
	if (error.code !== "ENOENT") {
		problems.push(`${TAURI_CONF}: could not be read (${error.message})`);
	}
}
if (tauriConf && Object.hasOwn(tauriConf, "version")) {
	found.push({ path: TAURI_CONF, value: tauriConf.version });
}

for (const { path, value } of found) {
	if (typeof value !== "string" || !VERSION.test(value)) {
		problems.push(`${path}: ${show(value)} is not a version like 1.2.3`);
	}
}

if (problems.length > 0) {
	fail(["version check failed:", ...problems.map((p) => `  ${p}`)]);
}

const version = found[0].value;
if (found.some(({ value }) => value !== version)) {
	fail([
		"version check failed: the version fields differ:",
		...found.map(({ path, value }) => `  ${path}: ${value}`),
	]);
}

if (tag !== undefined && tag !== `v${version}`) {
	fail([
		`version check failed: tag ${JSON.stringify(tag)} does not match version ${version} (expected v${version})`,
	]);
}

console.log(version);
