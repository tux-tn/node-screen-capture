# @screen-capture/node

Cross-platform native screen capture for Node.js. Windows uses [`windows-capture` 2.0.0](https://github.com/NiiightmareXD/windows-capture); Linux/Wayland uses the XDG ScreenCast portal and PipeWire.

## Table of contents

- [Requirements](#requirements)
- [Platform support](#platform-support)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Screen capture](#screen-capture)
  - [`ScreenCapture`](#screencapture)
  - [Capture targets and options](#capture-targets-and-options)
  - [Capability checks](#capability-checks)
- [Frames](#frames)
- [Monitor and window discovery (Windows only)](#monitor-and-window-discovery-windows-only)
  - [Monitor functions](#monitor-functions)
  - [Window functions](#window-functions)
- [DXGI Desktop Duplication](#dxgi-desktop-duplication)
- [Image encoding](#image-encoding)
  - [`ImageEncoder`](#imageencoder)
- [Video encoding (Windows only)](#video-encoding-windows-only)
  - [`VideoEncoder`](#videoencoder)
- [Constants and supported values](#constants-and-supported-values)
- [Rust compatibility](#rust-compatibility)
- [Contributing](#contributing)
- [License](#license)

## Requirements

- Node.js 20.17+, 22.13+, or 23.5+
- Windows 10 version 1903 or newer with the MSVC runtime, or
- Linux with Wayland, PipeWire 0.3, and a working `xdg-desktop-portal` ScreenCast backend

Check `isSupported()` and `captureApiSupport()` before starting capture. Wayland always delegates source selection to the desktop portal.

## Platform support

| Capability | Windows | Linux/Wayland |
| --- | --- | --- |
| Screen or window capture | Yes | Yes, through the XDG ScreenCast portal |
| Promise and async-iterator frame API | Yes | Yes |
| Cursor capture | Yes | Yes, when the portal supports it |
| Monitor and window discovery | Yes | No; Wayland intentionally does not expose global source enumeration |
| Target selection by index, title, or native handle | Yes | No; the portal owns source selection |
| Frame crop and packed RGBA/BGRA buffers | Yes | Yes |
| JPEG, PNG, GIF, TIFF, and BMP encoding | Yes | Yes |
| JPEG XR and RGB16F image encoding | Yes | No |
| DXGI Desktop Duplication | Yes | No |
| Video encoding | Yes | No |

The public TypeScript surface is shared across both backends. `ScreenCapture` is the platform-neutral capture class. Unsupported platform-specific methods throw an explicit error instead of silently changing behavior.

On Wayland, every new capture session opens the desktop portal. The user chooses the actual monitor or window; applications cannot bypass that consent dialog or enumerate sources ahead of time.

## Installation

```bash
npm install @screen-capture/node
```

```bash
pnpm add @screen-capture/node
```

## Quick start

Capture the primary monitor, save one PNG, and stop the native session:

```js
import {
  ImageFormat,
  ScreenCapture,
} from '@screen-capture/node'

const capture = new ScreenCapture({
  monitorIndex: 1,
  colorFormat: 'bgra8',
})

await capture.start()
const frame = await capture.nextFrame()

if (frame) {
  console.log({
    width: frame.width,
    height: frame.height,
    timestamp: frame.timestamp,
  })
  frame.saveAsImage('screenshot.png', ImageFormat.Png)
}

await capture.stop()
```

`monitorIndex` is one-based on Windows. On Wayland, the same call requests a monitor and the portal asks the user which monitor to share.

## Screen capture

### `ScreenCapture`

```ts
class ScreenCapture implements AsyncIterable<Frame> {
  constructor(options: CaptureOptions)
  start(): Promise<void>
  nextFrame(): Promise<Frame | undefined>
  stop(): Promise<void>
}
```

- `start()` starts the native capture session. Calling it twice without stopping rejects.
- `nextFrame()` waits for the latest available frame. It resolves to `undefined` after `stop()` or when the capture item closes.
- Only one `nextFrame()` call may be pending at a time.
- The JavaScript queue keeps at most one pending frame; older frames are dropped instead of accumulating buffers.
- `stop()` joins the native capture thread and closes pending frame reads.
- The async iterator starts and stops the session automatically:

```js
import { ScreenCapture } from '@screen-capture/node'

const capture = new ScreenCapture({ monitorIndex: 1 })

for await (const frame of capture) {
  processFrame(frame)
  if (shouldStop()) break
}
```

### Capture targets and options

Exactly one target may be selected. If all target fields are omitted, Windows captures monitor 1 and Wayland requests a monitor through the portal.

```ts
interface CaptureOptions {
  monitorIndex?: number
  windowName?: string
  windowHandle?: number
  usePicker?: boolean
  cursorCapture?: boolean
  drawBorder?: boolean
  includeSecondaryWindows?: boolean
  minimumUpdateIntervalMs?: number
  dirtyRegions?: boolean
  colorFormat?: ColorFormat
}
```

| Option | Description |
| --- | --- |
| `monitorIndex` | Windows: one-based monitor index. Wayland: requests a monitor; the numeric index cannot select it. |
| `windowName` | Windows: captures the first top-level window whose title contains this string. Wayland: requests a window; the title cannot select it. |
| `windowHandle` | Windows: captures a native `HWND`. Wayland: requests a window; the handle cannot select it. |
| `usePicker` | Opens the native Windows picker or allows monitor/window selection in the Wayland portal. |
| `cursorCapture` | Includes or excludes the cursor when supported. |
| `drawBorder` | Windows-only capture-border control. |
| `includeSecondaryWindows` | Windows-only secondary-window control. |
| `minimumUpdateIntervalMs` | Windows-only minimum interval between updates. |
| `dirtyRegions` | Windows-only dirty-region reporting and rendering control. |
| `colorFormat` | Requested frame format. Defaults to `ColorFormat.Bgra8`; Wayland supports `rgba8` and `bgra8`. |

Examples of each target:

```js
import { ScreenCapture } from '@screen-capture/node'

new ScreenCapture({ monitorIndex: 2 })
new ScreenCapture({ windowName: 'Visual Studio Code' })
new ScreenCapture({ windowHandle: hwnd })
new ScreenCapture({ usePicker: true })
```

### Capability checks

```ts
interface CaptureApiSupport {
  graphicsCapture: boolean
  cursorSettings: boolean
  borderSettings: boolean
  secondaryWindows: boolean
  minimumUpdateInterval: boolean
  dirtyRegions: boolean
}
```

```js
import {
  captureApiSupport,
  isSupported,
} from '@screen-capture/node'

if (!isSupported()) {
  throw new Error('Native screen capture is unavailable')
}

console.log(captureApiSupport())
```

`isSupported()` checks the native capture backend for the current platform. `captureApiSupport()` reports the core backend and each optional setting independently.

## Frames

```ts
class Frame {
  readonly buffer: Buffer
  readonly width: number
  readonly height: number
  readonly rowPitch: number
  readonly depthPitch: number
  readonly timestamp: number
  readonly colorFormat: ColorFormat
  readonly dirtyRegions: DirtyRegion[]

  crop(startX: number, startY: number, endX: number, endY: number): Frame
  encode(format: ImageFormat): Buffer
  saveAsImage(path: string, format?: ImageFormat): void
}

interface DirtyRegion {
  x: number
  y: number
  width: number
  height: number
}
```

- `buffer` contains packed pixel bytes with GPU row padding removed.
- `rowPitch` is the packed byte count per row.
- `depthPitch` is the packed byte count for the entire frame.
- `timestamp` uses 100-nanosecond ticks. Windows uses the platform capture clock; Wayland timestamps are relative to the current capture session.
- `dirtyRegions` contains changed rectangles on Windows when reporting is enabled. Wayland currently returns an empty array.
- `crop()` uses an exclusive end coordinate. Invalid rectangles throw.
- `encode()` and `saveAsImage()` support `rgba8` and `bgra8`. `rgba16F` frames must be converted before image encoding.
- `saveAsImage()` defaults to PNG when `format` is omitted.

```js
import {
  ImageFormat,
  ScreenCapture,
} from '@screen-capture/node'

const capture = new ScreenCapture({ monitorIndex: 1 })
await capture.start()
const frame = await capture.nextFrame()

if (frame) {
  const cropped = frame.crop(100, 100, 900, 700)
  const png = cropped.encode(ImageFormat.Png)
  cropped.saveAsImage('crop.png', ImageFormat.Png)
  console.log(png.byteLength)
}

await capture.stop()
```

## Monitor and window discovery (Windows only)

### Monitor functions

```ts
function enumerateMonitors(): MonitorInfo[]
function primaryMonitor(): MonitorInfo
function monitorFromIndex(index: number): MonitorInfo

interface MonitorInfo {
  index: number
  name: string
  deviceName: string
  deviceString: string
  width: number
  height: number
  refreshRate: number
  handle: number
}
```

These functions are Windows-only. `index` is one-based and `handle` is the native `HMONITOR` represented as a JavaScript number. Wayland callers receive an explicit unsupported error because source enumeration is not available through the ScreenCast portal.

### Window functions

```ts
function enumerateWindows(): WindowInfo[]
function foregroundWindow(): WindowInfo
function windowFromName(title: string): WindowInfo
function windowFromContainsName(title: string): WindowInfo
function windowFromHandle(handle: number): WindowInfo

interface WindowInfo {
  title: string
  processId: number
  processName: string
  rect: Rect
  titleBarHeight: number
  width: number
  height: number
  isValid: boolean
  handle: number
  monitorIndex?: number
}

interface Rect {
  left: number
  top: number
  right: number
  bottom: number
}
```

```js
import {
  enumerateMonitors,
  enumerateWindows,
  foregroundWindow,
  monitorFromIndex,
  primaryMonitor,
  windowFromContainsName,
  windowFromHandle,
  windowFromName,
} from '@screen-capture/node'

console.table(enumerateMonitors())
console.table(enumerateWindows())
console.log(primaryMonitor())
console.log(monitorFromIndex(1))
console.log(foregroundWindow())
console.log(windowFromName('Notepad'))
console.log(windowFromContainsName('Visual Studio'))
console.log(windowFromHandle(hwnd))
```

## DXGI Desktop Duplication

DXGI Desktop Duplication is a Windows-only synchronous API. On Wayland, use `ScreenCapture`, which receives frames asynchronously from PipeWire after portal selection.

```ts
class DxgiDuplicationSession {
  constructor(options?: DxgiSessionOptions)
  readonly width: number
  readonly height: number
  readonly format: DxgiDuplicationFormat
  readonly refreshRate: number[]
  acquireNextFrame(timeoutMs?: number): Frame | null
  recreate(): void
  switchMonitor(monitorIndex: number): void
}

interface DxgiSessionOptions {
  monitorIndex?: number
  supportedFormats?: DxgiDuplicationFormat[]
}
```

- `monitorIndex` defaults to monitor 1.
- `supportedFormats` is optional. If omitted, the native crate negotiates a supported format.
- `acquireNextFrame()` returns `null` when no desktop update arrives before the timeout. The default timeout is 16 ms.
- `recreate()` rebuilds the session after display-mode changes or DXGI access loss.
- `switchMonitor()` recreates the session on another one-based monitor index.
- `refreshRate` is `[numerator, denominator]`.

```js
import {
  DxgiDuplicationFormat,
  DxgiDuplicationSession,
  ImageFormat,
} from '@screen-capture/node'

const session = new DxgiDuplicationSession({
  monitorIndex: 1,
  supportedFormats: [
    DxgiDuplicationFormat.Bgra8,
    DxgiDuplicationFormat.Rgba8,
  ],
})

const frame = session.acquireNextFrame(33)
if (frame) {
  frame.saveAsImage('desktop.png', ImageFormat.Png)
}

// Call after a display mode or desktop switch if the session reports access loss.
session.recreate()
session.switchMonitor(2)
```

## Image encoding

### `ImageEncoder`

Use `ImageEncoder` for raw pixels that do not come from a `Frame`.

```ts
class ImageEncoder {
  constructor(format: ImageFormat, pixelFormat: ImageEncoderPixelFormat)
  encode(buffer: Buffer, width: number, height: number): Buffer
}
```

```js
import {
  ImageEncoder,
  ImageEncoderPixelFormat,
  ImageFormat,
} from '@screen-capture/node'

const encoder = new ImageEncoder(
  ImageFormat.Png,
  ImageEncoderPixelFormat.Bgra8,
)

const png = encoder.encode(rawBgraBuffer, width, height)
```

JPEG XR and RGB16F image encoding are Windows-only. The Wayland backend supports JPEG, PNG, GIF, TIFF, and BMP from packed `rgba8` or `bgra8` pixels.

## Video encoding (Windows only)

### `VideoEncoder`

`VideoEncoder` uses Windows Media Foundation. Constructing it on Wayland throws an explicit unsupported error.

```ts
class VideoEncoder {
  constructor(options: VideoEncoderOptions)
  sendFrame(frame: Frame): void
  sendFrameWithAudio(frame: Frame, audioBuffer: Buffer): void
  sendFrameBuffer(buffer: Buffer, timestamp: number): void
  sendAudioBuffer(buffer: Buffer, timestamp?: number): void
  finish(): Buffer | null
}

interface VideoEncoderOptions {
  path?: string
  video: VideoSettings
  audio?: AudioSettings
  container?: ContainerFormat
}

interface VideoSettings {
  width: number
  height: number
  codec?: VideoCodec
  bitrate?: number
  frameRate?: number
  pixelAspectRatioNumerator?: number
  pixelAspectRatioDenominator?: number
  disabled?: boolean
}

interface AudioSettings {
  codec?: AudioCodec
  bitrate?: number
  channelCount?: number
  sampleRate?: number
  bitsPerSample?: number
  disabled?: boolean
}
```

- Set `path` for file output. `finish()` returns `null` for file output.
- Omit `path` for in-memory output. `finish()` returns the encoded `Buffer`.
- Audio is disabled when `audio` is omitted.
- `sendFrame()` and `sendFrameWithAudio()` use the frame's timestamp.
- `sendAudioBuffer()` uses the encoder's audio clock; its optional timestamp is accepted for API compatibility.
- Always call `finish()` to flush and finalize the container.

```js
import {
  ContainerFormat,
  VideoCodec,
  VideoEncoder,
  ScreenCapture,
} from '@screen-capture/node'

const capture = new ScreenCapture({ monitorIndex: 1 })
await capture.start()
const firstFrame = await capture.nextFrame()

if (firstFrame) {
  const encoder = new VideoEncoder({
    path: 'capture.mp4',
    video: {
      width: firstFrame.width,
      height: firstFrame.height,
      codec: VideoCodec.H264,
      bitrate: 15_000_000,
      frameRate: 60,
    },
    container: ContainerFormat.Mpeg4,
  })

  try {
    encoder.sendFrame(firstFrame)
    const nextFrame = await capture.nextFrame()
    if (nextFrame) encoder.sendFrame(nextFrame)
  } finally {
    await capture.stop()
    encoder.finish()
  }
}
```

Raw `sendFrameBuffer()` input must be packed BGRA with the layout expected by the underlying Windows Media Foundation encoder. The Rust crate documents that raw buffers use bottom-to-top row order. `sendFrame(frame)` in this binding extracts the Node frame buffer and uses the same raw-buffer path; use the PNG path above when validating frame orientation.

In-memory output:

```js
import {
  ContainerFormat,
  VideoEncoder,
} from '@screen-capture/node'

const encoder = new VideoEncoder({
  video: { width: 1280, height: 720 },
  container: ContainerFormat.Mpeg4,
})

encoder.sendFrameBuffer(bgraBuffer, timestamp)
const encodedVideo = encoder.finish()
```

Audio input is interleaved PCM:

```js
import {
  AudioCodec,
  ContainerFormat,
  VideoCodec,
  VideoEncoder,
} from '@screen-capture/node'

const encoder = new VideoEncoder({
  path: 'capture-with-audio.mp4',
  video: {
    width: 1920,
    height: 1080,
    codec: VideoCodec.H264,
    frameRate: 60,
  },
  audio: {
    codec: AudioCodec.Aac,
    bitrate: 192_000,
    channelCount: 2,
    sampleRate: 48_000,
    bitsPerSample: 16,
  },
  container: ContainerFormat.Mpeg4,
})

encoder.sendFrameWithAudio(frame, pcmBuffer)
encoder.finish()
```

## Constants and supported values

### `ColorFormat`

`Rgba16F`, `Rgba8`, `Bgra8`

### `ImageFormat`

`Jpeg`, `Png`, `Gif`, `Tiff`, `Bmp`, `JpegXr` (`JpegXr` is Windows-only)

### `ImageEncoderPixelFormat`

`Rgb16F`, `Bgra8`, `Rgba8` (`Rgb16F` is Windows-only)

### `DxgiDuplicationFormat`

`Rgba16F`, `Rgb10A2`, `Rgb10XrA2`, `Rgba8`, `Rgba8Srgb`, `Bgra8`, `Bgra8Srgb`

### `VideoCodec`

`Argb32`, `Bgra8`, `D16`, `H263`, `H264`, `H264Es`, `Hevc`, `HevcEs`, `Iyuv`, `L8`, `L16`, `Mjpg`, `Nv12`, `Mpeg1`, `Mpeg2`, `Rgb24`, `Rgb32`, `Wmv3`, `Wvc1`, `Vp9`, `Yuy2`, `Yv12`

### `AudioCodec`

`Aac`, `Ac3`, `AacAdts`, `AacHdcp`, `Ac3Spdif`, `Ac3Hdcp`, `Adts`, `Alac`, `AmrNb`, `AwrWb`, `Dts`, `Eac3`, `Flac`, `Float`, `Mp3`, `Mpeg`, `Opus`, `Pcm`, `Wma8`, `Wma9`, `Vorbis`

### `ContainerFormat`

`Asf`, `Mp3`, `Mpeg4`, `Avi`, `Mpeg2`, `Wave`, `AacAdts`, `Adts`, `ThreeGp`, `Amr`, `Flac`

## Rust compatibility

On Windows, the binding follows the application-facing portions of [`windows-capture` 2.0.0](https://docs.rs/windows-capture/2.0.0/windows_capture/):

| Rust capability | Node.js API |
| --- | --- |
| Graphics Capture API and `Settings` | `ScreenCapture` and `CaptureOptions` |
| `GraphicsCaptureApiHandler` frame lifecycle | `Frame`, `nextFrame()`, and async iteration |
| Capture thread lifecycle | Promise-based `ScreenCapture.start()`, `nextFrame()`, and `stop()` |
| `Frame` and `FrameBuffer` | `Frame` and packed Node.js `Buffer` data |
| `Monitor` and `Window` | Monitor and window discovery functions |
| `GraphicsCapturePicker` | `usePicker: true` |
| `DxgiDuplicationApi` | `DxgiDuplicationSession` |
| `ImageEncoder` | `ImageEncoder`, `Frame.encode()`, and `Frame.saveAsImage()` |
| `VideoEncoder` | `VideoEncoder`, audio settings, codecs, and containers |
| Graphics Capture feature probes | `isSupported()` and `captureApiSupport()` |

On Linux, `ScreenCapture` implements the same promise and async-iterator lifecycle with the XDG ScreenCast portal and PipeWire. The portal's security model prevents parity for source discovery and direct source selection; DXGI and Windows Media Foundation APIs remain Windows-only.

Raw COM interfaces, D3D11 devices, textures, and surfaces are intentionally not exposed to JavaScript. Frames are represented as owned Node.js buffers so their lifetime is safe across the promise and async-iterator boundaries.

## Development

```bash
pnpm install
pnpm build:addon
pnpm build
pnpm lint
pnpm format
pnpm test
```

Rust formatting uses `cargo fmt`; TypeScript and JavaScript formatting use Oxfmt; linting uses Oxlint. Native builds require the Windows MSVC toolchain or Linux PipeWire development files.

## Contributing

Contributions are welcome. Native capture changes must be tested on the affected platform with the Rust toolchain and native development libraries.

### Development setup

```bash
pnpm install
```

Run the checks used by the project before opening a pull request:

```bash
pnpm build
pnpm lint
pnpm format
pnpm test
cargo fmt --all -- --check
```


### Pull requests

- Keep changes focused and update the README when public APIs change.
- Add or update Vitest coverage for observable JavaScript behavior.
- Keep generated NAPI artifacts synchronized with native API changes.
- Run the relevant Windows or Wayland capture checks for platform-specific changes.
- Include a concise description of the behavior changed and the verification performed.

## License

This software is released under the [MIT License](LICENSE).
