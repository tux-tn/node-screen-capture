import type * as Native from "../index.d.ts";
import { LatestFrameQueue } from "./frame-queue.js";
import * as addon from "../index.js";

export const {
	AudioCodec,
	ColorFormat,
	ContainerFormat,
	DxgiDuplicationFormat,
	DxgiDuplicationSession,
	Frame,
	ImageEncoder,
	ImageEncoderPixelFormat,
	ImageFormat,
	VideoCodec,
	VideoEncoder,
	captureApiSupport,
	enumerateMonitors,
	enumerateWindows,
	foregroundWindow,
	isSupported,
	monitorFromIndex,
	primaryMonitor,
	windowFromContainsName,
	windowFromHandle,
	windowFromName,
} = addon;

const NativeScreenCapture = addon.ScreenCapture;

export class ScreenCapture implements AsyncIterable<Native.Frame> {
	readonly #options: Native.CaptureOptions;
	#control: Native.CaptureControl | undefined;
	readonly #frames = new LatestFrameQueue<Native.Frame>();

	constructor(options: Native.CaptureOptions) {
		this.#options = options;
	}

	start(): Promise<void> {
		if (this.#control) {
			return Promise.reject(new Error("Capture session is already started"));
		}
		this.#frames.open();
		try {
			const capture = new NativeScreenCapture(
				this.#options,
				(frame: Native.Frame) => this.#frames.push(frame),
				() => this.#frames.close(),
			);
			this.#control = capture.start();
			return Promise.resolve();
		} catch (cause) {
			this.#frames.close();
			return Promise.reject(cause);
		}
	}

	nextFrame(): Promise<Native.Frame | undefined> {
		return this.#frames.next();
	}

	async stop(): Promise<void> {
		try {
			await this.#control?.stop();
		} finally {
			this.#control = undefined;
			this.#frames.close();
		}
	}

	async *[Symbol.asyncIterator](): AsyncIterator<Native.Frame> {
		if (!this.#control) {
			await this.start();
		}
		try {
			while (true) {
				const frame = await this.nextFrame();
				if (!frame) return;
				yield frame;
			}
		} finally {
			await this.stop();
		}
	}
}

export type AudioCodec = Native.AudioCodec;
export type ColorFormat = Native.ColorFormat;
export type ContainerFormat = Native.ContainerFormat;
export type DxgiDuplicationFormat = Native.DxgiDuplicationFormat;
export type DxgiDuplicationSession = Native.DxgiDuplicationSession;
export type Frame = Native.Frame;
export type ImageEncoder = Native.ImageEncoder;
export type ImageEncoderPixelFormat = Native.ImageEncoderPixelFormat;
export type ImageFormat = Native.ImageFormat;
export type VideoCodec = Native.VideoCodec;
export type VideoEncoder = Native.VideoEncoder;

export type {
	AudioSettings,
	CaptureApiSupport,
	CaptureOptions,
	DirtyRegion,
	DxgiSessionOptions,
	MonitorInfo,
	Rect,
	VideoEncoderOptions,
	VideoSettings,
	WindowInfo,
} from "../index.js";
