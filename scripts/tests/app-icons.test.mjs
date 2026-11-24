// @vitest-environment node
// Feature 13 AC20 and AC21: the committed app icon files, read from disk.
// The .icns is packed by macOS iconutil and is checked for its entries only.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { repoRoot } from "./helpers.mjs";
import { decodePng, isPng, pngSize } from "./png.mjs";

const icons = join(repoRoot, "src-tauri/icons");
const read = (name) => readFileSync(join(icons, name));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");

// The placeholder files from the project skeleton, which this feature
// replaces: the SHA-256 of each as committed before feature 13.
const PLACEHOLDERS = {
	"icon.png":
		"a17d73b902b012b33f29addfb5bf72cf7619157f04ed8bd1b844555b4e1fee09",
	"32x32.png":
		"b17deecb1d6f1dd6afb4d84aaf5919b260560eca8bf40909824274a40b1da483",
	"128x128.png":
		"bb1dc858364ffe1b438560d56e3c8dc3a23396ee97ef859bb90b4639113feb6e",
	"128x128@2x.png":
		"1f8d8a96fd8937a7c9f0d70b9c452f589aa711afd506cdaa7bf10aad307f5850",
	"icon.icns":
		"4b7674a75ca96a34023f044cbf6fd9e4bfdf622f8a929951e7bc8cba0c2db585",
	"icon.ico":
		"87b82c723f23c541e45a2c576b03a74a0baa0f60caecd601306419c57eb06c3a",
};

const PNG_FILES = [
	["icon.png", 1024],
	["32x32.png", 32],
	["128x128.png", 128],
	["128x128@2x.png", 256],
];

// The entries of an .ico as { w, bytes }.
function icoEntries(ico) {
	expect([...ico.subarray(0, 4)]).toEqual([0, 0, 1, 0]);
	const count = ico.readUInt16LE(4);
	const entries = [];
	for (let i = 0; i < count; i++) {
		const e = 6 + i * 16;
		const w = ico[e] === 0 ? 256 : ico[e];
		const len = ico.readUInt32LE(e + 8);
		const at = ico.readUInt32LE(e + 12);
		entries.push({ w, bytes: ico.subarray(at, at + len) });
	}
	return entries;
}

// The pixel sizes of the entries of an .icns. PNG entries say their own
// size; the older raw entry types are looked up; mask entries are skipped.
const RAW_SIZES = {
	is32: 16,
	il32: 32,
	ih32: 48,
	it32: 128,
	ic04: 16,
	ic05: 32,
};
function icnsSizes(icns) {
	expect(icns.toString("latin1", 0, 4)).toBe("icns");
	expect(icns.readUInt32BE(4)).toBe(icns.length);
	const sizes = [];
	let at = 8;
	while (at < icns.length) {
		const type = icns.toString("latin1", at, at + 4);
		const len = icns.readUInt32BE(at + 4);
		const data = icns.subarray(at + 8, at + len);
		if (isPng(data)) sizes.push(pngSize(data).w);
		else if (type in RAW_SIZES) sizes.push(RAW_SIZES[type]);
		at += len;
	}
	return sizes;
}

function isWhite(p) {
	return p[3] >= 200 && p[0] >= 200 && p[1] >= 200 && p[2] >= 200;
}

// The row through the middle of the mark: the middle of the rows that hold
// white pixels.
function markRow(img) {
	let first = -1;
	let last = -1;
	for (let y = 0; y < img.h; y++) {
		for (let x = 0; x < img.w; x++) {
			if (isWhite(img.px(x, y))) {
				if (first < 0) first = y;
				last = y;
				break;
			}
		}
	}
	expect(first, "the image has a white mark").toBeGreaterThanOrEqual(0);
	return Math.floor((first + last) / 2);
}

function whiteRuns(img, y) {
	const runs = [];
	for (let x = 0; x < img.w; x++) {
		if (isWhite(img.px(x, y))) {
			const run = runs[runs.length - 1];
			if (run && run[1] === x - 1) run[1] = x;
			else runs.push([x, x]);
		}
	}
	return runs;
}

describe("AC20 the app icon files", () => {
	for (const [name, size] of PNG_FILES) {
		it(`AC20 ${name} is a ${size} by ${size} PNG`, () => {
			expect(pngSize(read(name))).toEqual({ w: size, h: size });
		});
	}

	it("AC20 icon.icns holds sizes from 16 to 1024 pixels", () => {
		const sizes = new Set(icnsSizes(read("icon.icns")));
		expect(Math.min(...sizes)).toBe(16);
		expect(Math.max(...sizes)).toBe(1024);
		for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
			expect(sizes, `a ${size} pixel entry`).toContain(size);
		}
	});

	it("AC20 icon.ico holds at least 16, 24, 32, 48, 64 and 256 pixels", () => {
		const widths = icoEntries(read("icon.ico")).map((e) => e.w);
		for (const size of [16, 24, 32, 48, 64, 256]) {
			expect(widths, `a ${size} pixel entry`).toContain(size);
		}
	});

	it("AC20 each icon.ico entry is a PNG of the size it says", () => {
		for (const { w, bytes } of icoEntries(read("icon.ico"))) {
			expect(isPng(bytes), `entry ${w} is a PNG`).toBe(true);
			expect(pngSize(bytes)).toEqual({ w, h: w });
		}
	});

	it("AC20 every PNG has four transparent corners", () => {
		const images = PNG_FILES.map(([name]) => [name, read(name)]);
		for (const { w, bytes } of icoEntries(read("icon.ico"))) {
			images.push([`icon.ico ${w}`, bytes]);
		}
		for (const [name, bytes] of images) {
			const img = decodePng(bytes);
			for (const [x, y] of [
				[0, 0],
				[img.w - 1, 0],
				[0, img.h - 1],
				[img.w - 1, img.h - 1],
			]) {
				expect(img.px(x, y)[3], `${name} (${x}, ${y})`).toBe(0);
			}
		}
	});

	for (const name of Object.keys(PLACEHOLDERS)) {
		it(`AC20 ${name} is not the placeholder`, () => {
			expect(sha(read(name))).not.toBe(PLACEHOLDERS[name]);
		});
	}
});

describe("AC21 the committed 1024 drawing", () => {
	const img = decodePng(read("icon.png"));

	it("AC21 the centre of the tallest bar is white", () => {
		const row = markRow(img);
		const column = (x) => {
			const ys = [];
			for (let y = 0; y < img.h; y++) if (isWhite(img.px(x, y))) ys.push(y);
			return ys;
		};
		const a = column(511);
		const b = column(512);
		const tall = a.length > b.length ? a : b;
		expect(tall.length).toBeGreaterThan(100);
		const middle = Math.floor((tall[0] + tall[tall.length - 1]) / 2);
		expect(middle).toBe(row);
		expect(isWhite(img.px(511, middle)) || isWhite(img.px(512, middle))).toBe(
			true,
		);
		expect(middle, "the key top sits a little above centre").toBeLessThan(560);
	});

	it("AC21 a part of the body is the accent blue #2563eb", () => {
		let count = 0;
		for (let y = 0; y < img.h; y++) {
			for (let x = 0; x < img.w; x++) {
				const p = img.px(x, y);
				if (
					p[3] === 255 &&
					Math.abs(p[0] - 0x25) <= 2 &&
					Math.abs(p[1] - 0x63) <= 2 &&
					Math.abs(p[2] - 0xeb) <= 2
				) {
					count++;
				}
			}
		}
		expect(count).toBeGreaterThan((img.w * img.h) / 50);
	});

	it("AC21 the drawing is symmetrical left to right within a small tolerance", () => {
		let off = 0;
		let total = 0;
		for (let y = 0; y < img.h; y++) {
			for (let x = 0; x < img.w / 2; x++) {
				const a = img.px(x, y);
				const b = img.px(img.w - 1 - x, y);
				if (a[3] === 0 && b[3] === 0) continue;
				total++;
				if (a.some((v, c) => Math.abs(v - b[c]) > 16)) off++;
			}
		}
		expect(total).toBeGreaterThan(100000);
		expect(off / total).toBeLessThanOrEqual(0.01);
	});

	it("AC21 shows five bars along the middle row", () => {
		expect(whiteRuns(img, markRow(img))).toHaveLength(5);
	});
});

describe("AC21 the small drawings", () => {
	const entries = icoEntries(read("icon.ico"));
	const small = [
		["icon.ico 16", entries.find((e) => e.w === 16)?.bytes],
		["icon.ico 24", entries.find((e) => e.w === 24)?.bytes],
		["icon.ico 32", entries.find((e) => e.w === 32)?.bytes],
		["32x32.png", read("32x32.png")],
	];

	for (const [name, bytes] of small) {
		it(`AC21 ${name} shows three separate bars along the middle row`, () => {
			expect(bytes, `${name} exists`).toBeDefined();
			const img = decodePng(bytes);
			const y = markRow(img);
			const pattern = [];
			for (let x = 0; x < img.w; x++) {
				pattern.push(isWhite(img.px(x, y)) ? "W" : ".");
			}
			const squeezed = pattern
				.join("")
				.replace(/(.)\1+/g, "$1")
				.replace(/^\.|\.$/g, "");
			expect(squeezed, `row ${y}: ${pattern.join("")}`).toBe("W.W.W");
		});
	}

	it("AC21 the larger drawings keep five bars", () => {
		for (const [name, bytes] of [
			["128x128.png", read("128x128.png")],
			["128x128@2x.png", read("128x128@2x.png")],
		]) {
			const img = decodePng(bytes);
			expect(whiteRuns(img, markRow(img)), name).toHaveLength(5);
		}
	});
});
