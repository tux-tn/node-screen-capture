import { Buffer } from "node:buffer";

import { describe, expect, test } from "vitest";

import { ColorFormat, ImageFormat, ScreenCapture, isSupported } from "../../index.js";
import type { Frame } from "../../index.js";

const timeout = 30_000;

function waitFor<T>(promise: Promise<T>, message: string): Promise<T> {
	return Promise.race([
		promise,
		new Promise<T>((_, reject) => {
			setTimeout(() => reject(new Error(message)), timeout).unref();
		}),
	]);
}

describe("native capture e2e", () => {
	test("rejects capture options with more than one target", () => {
		expect(() => new ScreenCapture({ monitorIndex: 1, usePicker: true }, () => {})).toThrow(/Specify only one/);
	});

	test("delivers a frame and closed callback", async () => {
		if (!isSupported()) return;

		const isWindows = process.platform === "win32";
		let resolveFrame!: (frame: Frame) => void;
		let resolveClosed!: () => void;
		let settled = false;
		const framePromise = new Promise<Frame>((resolve) => {
			resolveFrame = (frame) => {
				if (!settled) {
					settled = true;
					resolve(frame);
				}
			};
		});
		const closedPromise = new Promise<void>((resolve) => {
			resolveClosed = resolve;
		});

		const capture = new ScreenCapture(
			isWindows
				? { monitorIndex: 1, colorFormat: ColorFormat.Bgra8 }
				: { usePicker: true, colorFormat: ColorFormat.Bgra8 },
			(frame) => resolveFrame(frame),
			() => resolveClosed(),
		);
		const control = capture.start();

		try {
			const frame = await waitFor(framePromise, "Timed out waiting for the first captured frame");
			expect(frame.width).toBeGreaterThan(0);
			expect(frame.height).toBeGreaterThan(0);
			expect(frame.colorFormat).toBe(ColorFormat.Bgra8);
			expect(frame.buffer.byteLength).toBeGreaterThanOrEqual(frame.width * frame.height * 4);
			expect(frame.encode(ImageFormat.Png).subarray(0, 8)).toEqual(
				Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
			);
		} finally {
			await control.stop();
		}

		await waitFor(closedPromise, "Timed out waiting for the closed callback");
		expect(control.isFinished).toBe(true);
	});
});
