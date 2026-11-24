// A small PNG reader for the tests of the committed icons: 8-bit RGBA only,
// any filter, no interlace. It uses only node:zlib.
import { inflateSync } from "node:zlib";

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

export function isPng(bytes) {
	return Buffer.from(bytes.subarray(0, 8)).equals(SIGNATURE);
}

// Width and height from the IHDR chunk, without decoding the pixels.
export function pngSize(bytes) {
	if (!isPng(bytes)) throw new Error("not a PNG");
	const b = Buffer.from(bytes);
	return { w: b.readUInt32BE(16), h: b.readUInt32BE(20) };
}

function paeth(a, b, c) {
	const p = a + b - c;
	const pa = Math.abs(p - a);
	const pb = Math.abs(p - b);
	const pc = Math.abs(p - c);
	if (pa <= pb && pa <= pc) return a;
	return pb <= pc ? b : c;
}

// Decodes an 8-bit RGBA PNG to { w, h, px(x, y) -> [r, g, b, a] }.
export function decodePng(bytes) {
	const b = Buffer.from(bytes);
	if (!isPng(b)) throw new Error("not a PNG");
	let at = 8;
	let w = 0;
	let h = 0;
	const idat = [];
	while (at < b.length) {
		const len = b.readUInt32BE(at);
		const kind = b.toString("latin1", at + 4, at + 8);
		const data = b.subarray(at + 8, at + 8 + len);
		if (kind === "IHDR") {
			w = data.readUInt32BE(0);
			h = data.readUInt32BE(4);
			if (data[8] !== 8 || data[9] !== 6 || data[12] !== 0) {
				throw new Error(
					`expected 8-bit RGBA, no interlace; got depth ${data[8]} type ${data[9]} interlace ${data[12]}`,
				);
			}
		} else if (kind === "IDAT") {
			idat.push(data);
		}
		at += 12 + len;
	}
	const raw = inflateSync(Buffer.concat(idat));
	const stride = w * 4;
	const out = Buffer.alloc(stride * h);
	for (let y = 0; y < h; y++) {
		const filter = raw[y * (stride + 1)];
		const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
		for (let i = 0; i < stride; i++) {
			const left = i >= 4 ? out[y * stride + i - 4] : 0;
			const up = y > 0 ? out[(y - 1) * stride + i] : 0;
			const upLeft = y > 0 && i >= 4 ? out[(y - 1) * stride + i - 4] : 0;
			let v = line[i];
			if (filter === 1) v += left;
			else if (filter === 2) v += up;
			else if (filter === 3) v += (left + up) >> 1;
			else if (filter === 4) v += paeth(left, up, upLeft);
			out[y * stride + i] = v & 255;
		}
	}
	return {
		w,
		h,
		px(x, y) {
			const o = (y * w + x) * 4;
			return [out[o], out[o + 1], out[o + 2], out[o + 3]];
		},
	};
}
