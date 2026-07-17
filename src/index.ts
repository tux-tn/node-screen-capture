import type * as Native from "../index.d.ts";
import { LatestFrameQueue } from "./frame-queue.js";
import * as addon from "../index.js";

export const {
	AudioCodec,
	ColorFormat,
	ContainerFormat,
	DxgiDuplicationFormat,
	ImageEncoderPixelFormat,
	ImageFormat,
	VideoCodec,
} = addon;
/**
 * Windows-only synchronous desktop duplication session.
 *
 * @param options DXGI monitor and format negotiation options.
 */
export const DxgiDuplicationSession: {
	new (options?: DxgiSessionOptions | null): DxgiDuplicationSession;
} = addon.DxgiDuplicationSession;
/** A captured frame and its packed pixel data. */
export const Frame = addon.Frame;
/**
 * Encodes raw packed pixels into an image buffer.
 *
 * @param format Output image format.
 * @param pixelFormat Input pixel format.
 */
export const ImageEncoder: {
	new (format: ImageFormat, pixelFormat: ImageEncoderPixelFormat): ImageEncoder;
} = addon.ImageEncoder;
/**
 * Windows-only Media Foundation video encoder.
 *
 * @param options Video, audio, container, and output settings.
 */
export const VideoEncoder: {
	new (options: VideoEncoderOptions): VideoEncoder;
} = addon.VideoEncoder;
/**
 * Reports capture capabilities for the current platform.
 *
 * @returns Core capture support and optional feature support.
 */
export const captureApiSupport = addon.captureApiSupport;
/**
 * Lists available monitors on Windows and macOS.
 *
 * @returns Monitors ordered by their one-based index.
 * @throws On Wayland, where global monitor discovery is unavailable.
 */
export const enumerateMonitors = addon.enumerateMonitors;
/**
 * Lists available top-level windows on Windows and macOS.
 *
 * @returns Information about each available window.
 * @throws On Wayland, where global window discovery is unavailable.
 */
export const enumerateWindows = addon.enumerateWindows;
/**
 * Returns the current foreground window on Windows and macOS.
 *
 * @throws On Wayland, where global window discovery is unavailable.
 */
export const foregroundWindow = addon.foregroundWindow;
/**
 * Reports whether the current platform's native capture backend is available.
 */
export const isSupported = addon.isSupported;
/**
 * Returns monitor information for a one-based index.
 *
 * @param index One-based monitor index.
 * @throws If the index is invalid or monitor discovery is unavailable.
 */
export const monitorFromIndex = addon.monitorFromIndex;
/**
 * Returns the primary monitor on Windows and macOS.
 *
 * @throws On Wayland, where global monitor discovery is unavailable.
 */
export const primaryMonitor = addon.primaryMonitor;
/**
 * Finds the first window whose title contains the supplied text.
 *
 * @param title Window-title substring.
 * @throws If no matching window exists or window discovery is unavailable.
 */
export const windowFromContainsName = addon.windowFromContainsName;
/**
 * Returns a window by its native handle.
 *
 * @param handle HWND on Windows or ScreenCaptureKit window ID on macOS.
 * @throws If the handle is invalid or window discovery is unavailable.
 */
export const windowFromHandle = addon.windowFromHandle;
/**
 * Finds the first top-level window with the supplied exact title.
 *
 * @param title Exact window title.
 * @throws If no matching window exists or window discovery is unavailable.
 */
export const windowFromName = addon.windowFromName;

const NativeScreenCapture = addon.ScreenCapture;

/**
 * Options for selecting a capture source and configuring a capture session.
 *
 * Select at most one of `monitorIndex`, `windowName`, `windowHandle`, or
 * `usePicker`. With no target, Windows and macOS capture monitor 1 while
 * Wayland opens the desktop portal.
 *
 * @see {@link https://github.com/tux-tn/node-screen-capture#capture-targets-and-options | Capture targets and options documentation}
 */
export interface CaptureOptions extends Native.CaptureOptions {
	/** One-based monitor index on Windows and macOS. */
	monitorIndex?: Native.CaptureOptions["monitorIndex"];
	/** Case-sensitive title substring to capture on Windows and macOS. */
	windowName?: Native.CaptureOptions["windowName"];
	/** Native HWND on Windows or ScreenCaptureKit window ID on macOS. */
	windowHandle?: Native.CaptureOptions["windowHandle"];
	/** Opens the native picker on Windows/macOS or the source picker on Wayland. */
	usePicker?: Native.CaptureOptions["usePicker"];
	/** Controls whether the cursor is included when supported. */
	cursorCapture?: Native.CaptureOptions["cursorCapture"];
	/** Controls the capture border on Windows. */
	drawBorder?: Native.CaptureOptions["drawBorder"];
	/** Controls inclusion of secondary windows on Windows. */
	includeSecondaryWindows?: Native.CaptureOptions["includeSecondaryWindows"];
	/** Sets the minimum interval between frame updates on Windows. */
	minimumUpdateIntervalMs?: Native.CaptureOptions["minimumUpdateIntervalMs"];
	/** Enables dirty-region reporting on Windows. */
	dirtyRegions?: Native.CaptureOptions["dirtyRegions"];
	/** Requested output pixel format; defaults to `Bgra8`. `Rgba16F` is Windows-only. */
	colorFormat?: Native.CaptureOptions["colorFormat"];
}

/** Options for a Windows DXGI Desktop Duplication session. */
export interface DxgiSessionOptions extends Native.DxgiSessionOptions {
	/** One-based monitor index to duplicate. */
	monitorIndex?: Native.DxgiSessionOptions["monitorIndex"];
	/** Formats offered during DXGI format negotiation. */
	supportedFormats?: Native.DxgiSessionOptions["supportedFormats"];
}

/** Windows Media Foundation audio stream settings. */
export interface AudioSettings extends Native.AudioSettings {
	/** Audio codec. */
	codec?: Native.AudioSettings["codec"];
	/** Target bitrate in bits per second. */
	bitrate?: Native.AudioSettings["bitrate"];
	/** Number of interleaved audio channels. */
	channelCount?: Native.AudioSettings["channelCount"];
	/** Sample rate in hertz. */
	sampleRate?: Native.AudioSettings["sampleRate"];
	/** Bits per PCM sample. */
	bitsPerSample?: Native.AudioSettings["bitsPerSample"];
	/** Disables the audio stream when `true`. */
	disabled?: Native.AudioSettings["disabled"];
}

/** Windows Media Foundation video stream settings. */
export interface VideoSettings extends Native.VideoSettings {
	/** Output width in pixels. */
	width: Native.VideoSettings["width"];
	/** Output height in pixels. */
	height: Native.VideoSettings["height"];
	/** Video codec. */
	codec?: Native.VideoSettings["codec"];
	/** Target bitrate in bits per second. */
	bitrate?: Native.VideoSettings["bitrate"];
	/** Target frames per second. */
	frameRate?: Native.VideoSettings["frameRate"];
	/** Pixel aspect-ratio numerator. */
	pixelAspectRatioNumerator?: Native.VideoSettings["pixelAspectRatioNumerator"];
	/** Pixel aspect-ratio denominator. */
	pixelAspectRatioDenominator?: Native.VideoSettings["pixelAspectRatioDenominator"];
	/** Disables the video stream when `true`. */
	disabled?: Native.VideoSettings["disabled"];
}

/** Options for the Windows Media Foundation video encoder. */
export interface VideoEncoderOptions extends Native.VideoEncoderOptions {
	/** Writes encoded output to this path; omit for in-memory output. */
	path?: Native.VideoEncoderOptions["path"];
	/** Video stream configuration. */
	video: VideoSettings;
	/** Optional interleaved PCM audio stream configuration. */
	audio?: AudioSettings;
	/** Output container format. */
	container?: Native.VideoEncoderOptions["container"];
}
/**
 * Coordinates native screen capture with a bounded, latest-frame queue.
 *
 * The async iterator starts the session when needed and stops it when the
 * iterator completes, throws, or is closed.
 */
export class ScreenCapture implements AsyncIterable<Native.Frame> {
	readonly #options: CaptureOptions;
	#control: Native.CaptureControl | undefined;
	readonly #frames = new LatestFrameQueue<Native.Frame>();

	/**
	 * Creates a capture session.
	 *
	 * @param options Capture target and backend settings. On Wayland, omit
	 * selectors to let the desktop portal choose the source.
	 *
	 * @see CaptureOptions
	 */
	constructor(options: CaptureOptions) {
		this.#options = options;
	}

	/**
	 * Starts the native capture session.
	 *
	 * Calling `start()` while the session is already running rejects.
	 * Synchronous native initialization failures reject this promise; errors
	 * reported after startup reject `nextFrame()`.
	 *
	 * @returns A promise that resolves after the native capture control is created.
	 */
	start(): Promise<void> {
		if (this.#control) {
			return Promise.reject(new Error("Capture session is already started"));
		}
		this.#frames.open();
		try {
			const capture = new NativeScreenCapture(
				this.#options,
				(frame: Native.Frame) => this.#frames.push(frame),
				(error: string | null) => {
					if (error) {
						this.#frames.fail(new Error(error));
					}
					this.#frames.close();
				},
			);
			this.#control = capture.start();
			return Promise.resolve();
		} catch (cause) {
			this.#frames.close();
			return Promise.reject(cause);
		}
	}

	/**
	 * Waits for the next available frame.
	 *
	 * The queue retains at most one latest frame, so older frames may be
	 * discarded when the consumer is slower than the capture source. Only one
	 * pending call is allowed at a time.
	 *
	 * @returns The next frame, or `undefined` before startup or after the session stops or closes.
	 */
	nextFrame(): Promise<Native.Frame | undefined> {
		return this.#frames.next();
	}

	/**
	 * Stops the native session and resolves any pending frame request.
	 *
	 * Calling `stop()` on an inactive session is safe.
	 *
	 * @returns A promise that resolves after the native session has stopped.
	 */
	async stop(): Promise<void> {
		try {
			await this.#control?.stop();
		} finally {
			this.#control = undefined;
			this.#frames.close();
		}
	}

	/**
	 * Iterates over frames until the session closes or iteration is stopped.
	 *
	 * The session starts automatically when iteration begins and is stopped
	 * automatically when the iterator exits.
	 */
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
/**
 * A DXGI desktop duplication session.
 */
export interface DxgiDuplicationSession extends Native.DxgiDuplicationSession {
	/** Width of the duplicated desktop in pixels. */
	readonly width: Native.DxgiDuplicationSession["width"];
	/** Height of the duplicated desktop in pixels. */
	readonly height: Native.DxgiDuplicationSession["height"];
	/** Negotiated desktop pixel format. */
	readonly format: Native.DxgiDuplicationSession["format"];
	/** Refresh-rate numerator and denominator. */
	readonly refreshRate: Native.DxgiDuplicationSession["refreshRate"];
	/**
	 * Acquires the next updated desktop frame.
	 *
	 * @param timeoutMs Timeout in milliseconds; defaults to 16.
	 * @returns A frame, or `null` when the timeout expires without an update.
	 */
	acquireNextFrame(timeoutMs?: number | null): Frame | null;
	/** Recreates the session after display changes or access loss. */
	recreate(): void;
	/**
	 * Recreates the session for a one-based monitor index.
	 *
	 * @param monitorIndex One-based monitor index.
	 */
	switchMonitor(monitorIndex: number): void;
}
/**
 * A captured frame with packed pixel data and capture metadata.
 */
export interface Frame extends Native.Frame {
	/** Packed pixel bytes with GPU row padding removed. */
	readonly buffer: Native.Frame["buffer"];
	/** Frame width in pixels. */
	readonly width: Native.Frame["width"];
	/** Frame height in pixels. */
	readonly height: Native.Frame["height"];
	/** Packed bytes per row. */
	readonly rowPitch: Native.Frame["rowPitch"];
	/** Packed bytes for the entire frame. */
	readonly depthPitch: Native.Frame["depthPitch"];
	/** Capture timestamp in 100-nanosecond ticks. */
	readonly timestamp: Native.Frame["timestamp"];
	/** Pixel format of the frame buffer. */
	readonly colorFormat: Native.Frame["colorFormat"];
	/** Dirty rectangles from Windows Graphics Capture when enabled; otherwise empty. */
	readonly dirtyRegions: Native.Frame["dirtyRegions"];
	/**
	 * Returns a frame containing an exclusive-end crop rectangle.
	 *
	 * @throws If the rectangle is empty or outside the frame bounds.
	 */
	crop(startX: number, startY: number, endX: number, endY: number): Frame;
	/** Encodes the frame using the requested image format. */
	encode(format: ImageFormat): Buffer;
	/** Saves the frame to an image file, defaulting to PNG. */
	saveAsImage(path: string, format?: ImageFormat | null): void;
}
export type ImageEncoderPixelFormat = Native.ImageEncoderPixelFormat;
export type ImageFormat = Native.ImageFormat;
/**
 * Encodes raw packed pixels into an image buffer.
 */
export interface ImageEncoder extends Native.ImageEncoder {
	/**
	 * Encodes a packed pixel buffer.
	 *
	 * @param buffer Packed pixels in the format selected by the constructor.
	 * @param width Image width in pixels.
	 * @param height Image height in pixels.
	 * @returns Encoded image bytes.
	 */
	encode(buffer: Buffer, width: number, height: number): Buffer;
}
export type VideoCodec = Native.VideoCodec;
/**
 * Encodes video and optional audio through Windows Media Foundation.
 */
export interface VideoEncoder extends Native.VideoEncoder {
	/** Adds a captured frame using its capture timestamp. */
	sendFrame(frame: Frame): void;
	/** Adds a captured frame and interleaved PCM audio with the frame timestamp. */
	sendFrameWithAudio(frame: Frame, audioBuffer: Buffer): void;
	/** Adds a packed BGRA frame in bottom-to-top row order. */
	sendFrameBuffer(buffer: Buffer, timestamp: number): void;
	/** Adds interleaved PCM audio, using timestamp `0` when omitted. */
	sendAudioBuffer(buffer: Buffer, timestamp?: number | null): void;
	/**
	 * Flushes and finalizes the output.
	 *
	 * @returns Encoded bytes for in-memory output, or `null` for file output.
	 */
	finish(): Buffer | null;
}

export type { CaptureApiSupport, DirtyRegion, MonitorInfo, Rect, WindowInfo } from "../index.js";
