import { describe, expect, test, vi } from "vitest";

interface TestFrame {
	id: number;
}

interface NativeControl {
	stop: () => Promise<void>;
}

interface NativeCapture {
	start: () => NativeControl;
}

const native = vi.hoisted(() => ({
	ScreenCapture: vi.fn(),
}));
vi.mock("../../index.js", () => ({
	AudioCodec: undefined,
	ColorFormat: undefined,
	ContainerFormat: undefined,
	DxgiDuplicationFormat: undefined,
	DxgiDuplicationSession: undefined,
	Frame: undefined,
	ImageEncoder: undefined,
	ImageEncoderPixelFormat: undefined,
	ImageFormat: undefined,
	VideoCodec: undefined,
	VideoEncoder: undefined,
	captureApiSupport: undefined,
	enumerateMonitors: undefined,
	enumerateWindows: undefined,
	foregroundWindow: undefined,
	isSupported: undefined,
	monitorFromIndex: undefined,
	primaryMonitor: undefined,
	windowFromContainsName: undefined,
	windowFromHandle: undefined,
	windowFromName: undefined,
	ScreenCapture: native.ScreenCapture,
}));

import { ScreenCapture } from "../../src/index.js";

describe("ScreenCapture wrapper", () => {
	test("delivers native frames and stops the native session", async () => {
		const frame: TestFrame = { id: 1 };
		let onFrame: (value: TestFrame) => void = () => {};
		let onClosed: (error: string | null) => void = () => {};
		const control: NativeControl = { stop: vi.fn(async () => onClosed(null)) };
		const nativeCapture: NativeCapture = {
			start: vi.fn(() => control),
		};
		native.ScreenCapture.mockImplementation(function (_options, frameCallback, closedCallback) {
			onFrame = frameCallback;
			onClosed = closedCallback;
			return nativeCapture;
		});

		const capture = new ScreenCapture({});
		await capture.start();
		const nextFrame = capture.nextFrame();
		onFrame(frame);
		await expect(nextFrame).resolves.toBe(frame);
		await capture.stop();
		await expect(capture.nextFrame()).resolves.toBeUndefined();
		expect(control.stop).toHaveBeenCalledOnce();
	});

	test("rejects a second start without creating another native session", async () => {
		const control: NativeControl = { stop: vi.fn(async () => {}) };
		const nativeCapture: NativeCapture = { start: vi.fn(() => control) };
		native.ScreenCapture.mockImplementation(function () {
			return nativeCapture;
		});

		const capture = new ScreenCapture({});
		await capture.start();

		await expect(capture.start()).rejects.toThrow("Capture session is already started");
		expect(native.ScreenCapture).toHaveBeenCalledOnce();
		await capture.stop();
	});

	test("closes the frame queue when native startup fails", async () => {
		const cause = new Error("native startup failed");
		native.ScreenCapture.mockImplementation(function () {
			throw cause;
		});

		const capture = new ScreenCapture({});

		await expect(capture.start()).rejects.toBe(cause);
		await expect(capture.nextFrame()).resolves.toBeUndefined();
	});

	test("starts and stops automatically through the async iterator", async () => {
		const frame: TestFrame = { id: 2 };
		let onFrame: (value: TestFrame) => void = () => {};
		let onClosed: (error: string | null) => void = () => {};
		const control: NativeControl = { stop: vi.fn(async () => onClosed(null)) };
		const nativeCapture: NativeCapture = {
			start: vi.fn(() => {
				onFrame(frame);
				return control;
			}),
		};
		native.ScreenCapture.mockImplementation(function (_options, frameCallback, closedCallback) {
			onFrame = frameCallback;
			onClosed = closedCallback;
			return nativeCapture;
		});

		const frames: TestFrame[] = [];
		for await (const value of new ScreenCapture({})) {
			frames.push(value);
			break;
		}

		expect(frames).toEqual([frame]);
		expect(control.stop).toHaveBeenCalledOnce();
	});

	test("rejects pending nextFrame when native close reports an error", async () => {
		let onClosed: (error: string | null) => void = () => {};
		const control: NativeControl = { stop: vi.fn(async () => {}) };
		const nativeCapture: NativeCapture = {
			start: vi.fn(() => control),
		};
		native.ScreenCapture.mockImplementation(function (_options, _frameCallback, closedCallback) {
			onClosed = closedCallback;
			return nativeCapture;
		});

		const capture = new ScreenCapture({});
		await capture.start();
		const pending = capture.nextFrame();

		onClosed("native stream error");

		await expect(pending).rejects.toThrow("native stream error");
	});

	test("rejects future nextFrame calls after native close error", async () => {
		let onClosed: (error: string | null) => void = () => {};
		const control: NativeControl = { stop: vi.fn(async () => {}) };
		const nativeCapture: NativeCapture = {
			start: vi.fn(() => control),
		};
		native.ScreenCapture.mockImplementation(function (_options, _frameCallback, closedCallback) {
			onClosed = closedCallback;
			return nativeCapture;
		});

		const capture = new ScreenCapture({});
		await capture.start();

		onClosed("native stream error");

		await expect(capture.nextFrame()).rejects.toThrow("native stream error");
	});

	test("resolves nextFrame as undefined when native close reports null", async () => {
		let onClosed: (error: string | null) => void = () => {};
		const control: NativeControl = { stop: vi.fn(async () => {}) };
		const nativeCapture: NativeCapture = {
			start: vi.fn(() => control),
		};
		native.ScreenCapture.mockImplementation(function (_options, _frameCallback, closedCallback) {
			onClosed = closedCallback;
			return nativeCapture;
		});

		const capture = new ScreenCapture({});
		await capture.start();
		const pending = capture.nextFrame();

		onClosed(null);

		await expect(pending).resolves.toBeUndefined();
	});
});
