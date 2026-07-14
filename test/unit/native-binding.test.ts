import { Buffer } from "node:buffer";

import { describe, expect, test } from "vitest";

import {
	ImageEncoder,
	ImageEncoderPixelFormat,
	ImageFormat,
	captureApiSupport,
	enumerateMonitors,
	isSupported,
} from "../../index.js";

describe("native binding", () => {
	test("reports capture capabilities as booleans", () => {
		expect(typeof isSupported()).toBe("boolean");
		expect(captureApiSupport()).toEqual({
			graphicsCapture: expect.any(Boolean),
			cursorSettings: expect.any(Boolean),
			borderSettings: expect.any(Boolean),
			secondaryWindows: expect.any(Boolean),
			minimumUpdateInterval: expect.any(Boolean),
			dirtyRegions: expect.any(Boolean),
		});
	});

	test("encodes packed RGBA pixels as PNG", () => {
		const encoder = new ImageEncoder(ImageFormat.Png, ImageEncoderPixelFormat.Rgba8);
		const encoded = encoder.encode(Buffer.from([255, 0, 0, 255]), 1, 1);

		expect(encoded.subarray(0, 8)).toEqual(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
	});

	test.runIf(process.platform === "linux")("rejects monitor enumeration on Wayland", () => {
		expect(() => enumerateMonitors()).toThrow(/unavailable on Wayland/i);
	});
});
