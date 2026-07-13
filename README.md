# @native-capture/windows

Windows screen-capture, desktop-duplication, image-encoding, and video-encoding APIs for Node.js. The package is a native NAPI binding for [`windows-capture` 2.0.0](https://github.com/NiiightmareXD/windows-capture).

## Table of contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Windows Graphics Capture](#windows-graphics-capture)
  - [`WindowsCapture`](#windowscapture)
  - [Capture targets and options](#capture-targets-and-options)
  - [Capability checks](#capability-checks)
- [Frames](#frames)
- [Monitor and window discovery](#monitor-and-window-discovery)
  - [Monitor functions](#monitor-functions)
  - [Window functions](#window-functions)
- [DXGI Desktop Duplication](#dxgi-desktop-duplication)
- [Image encoding](#image-encoding)
  - [`ImageEncoder`](#imageencoder)
- [Video encoding](#video-encoding)
  - [`VideoEncoder`](#videoencoder)
- [Constants and supported values](#constants-and-supported-values)
- [Rust compatibility](#rust-compatibility)
- [Contributing](#contributing)
- [License](#license)

## Requirements

- Windows 10 version 1903 or newer
- Node.js 20.17+, 22.13+, or 23.5+
- A Windows MSVC runtime matching the published native package

This package is Windows-only. Check `isSupported()` and `captureApiSupport()` before enabling optional Windows Graphics Capture features.

## Installation

```bash
npm install @native-capture/windows
```

```bash
pnpm add @native-capture/windows
```

## Quick start

Capture the primary monitor, save one PNG, and stop the native session:

```js
import {
  ImageFormat,
  WindowsCapture,
} from '@native-capture/windows'

const capture = new WindowsCapture({
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

`monitorIndex` is one-based. If no capture target is provided, monitor 1 is used.

## Windows Graphics Capture

### `WindowsCapture`

```ts
class WindowsCapture implements AsyncIterable<Frame> {
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
import { WindowsCapture } from '@native-capture/windows'

const capture = new WindowsCapture({ monitorIndex: 1 })

for await (const frame of capture) {
  processFrame(frame)
  if (shouldStop()) break
}
```

### Capture targets and options

Exactly one target may be selected. If all target fields are omitted, monitor 1 is captured.

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
| `monitorIndex` | One-based monitor index. |
| `windowName` | Captures the first top-level window whose title contains this string. |
| `windowHandle` | Captures a native `HWND` represented as a JavaScript number. |
| `usePicker` | Opens the Windows Graphics Capture picker. The picker runs on a dedicated native thread. |
| `cursorCapture` | `true` includes the cursor, `false` excludes it, and omission uses the Windows default. |
| `drawBorder` | `true` draws the capture border, `false` disables it, and omission uses the Windows default. |
| `includeSecondaryWindows` | Controls secondary-window capture. Omission uses the Windows default. |
| `minimumUpdateIntervalMs` | Minimum interval between updates. Omission uses the Windows default. |
| `dirtyRegions` | `true` reports and renders dirty regions, `false` reports only, and omission uses the Windows default. |
| `colorFormat` | Requested frame format. Defaults to `ColorFormat.Bgra8`. |

Examples of each target:

```js
import { WindowsCapture } from '@native-capture/windows'

new WindowsCapture({ monitorIndex: 2 })
new WindowsCapture({ windowName: 'Visual Studio Code' })
new WindowsCapture({ windowHandle: hwnd })
new WindowsCapture({ usePicker: true })
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
} from '@native-capture/windows'

if (!isSupported()) {
  throw new Error('Windows Graphics Capture is unavailable')
}

console.log(captureApiSupport())
```

`isSupported()` checks the core Graphics Capture API. `captureApiSupport()` reports support for the core API and each optional setting independently.

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
  saveAsImage(path: string, format: ImageFormat): void
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
- `timestamp` is the Windows Graphics Capture timestamp in 100-nanosecond ticks.
- `dirtyRegions` contains changed rectangles when dirty-region reporting is enabled.
- `crop()` uses an exclusive end coordinate. Invalid rectangles throw.
- `encode()` and `saveAsImage()` support `rgba8` and `bgra8`. `rgba16F` frames must be converted before image encoding.

```js
import {
  ImageFormat,
  WindowsCapture,
} from '@native-capture/windows'

const capture = new WindowsCapture({ monitorIndex: 1 })
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

## Monitor and window discovery

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

`index` is one-based. `handle` is the native `HMONITOR` represented as a JavaScript number.

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
} from '@native-capture/windows'

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

DXGI Desktop Duplication is synchronous and is useful when the caller controls its own capture loop.

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
} from '@native-capture/windows'

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
} from '@native-capture/windows'

const encoder = new ImageEncoder(
  ImageFormat.Png,
  ImageEncoderPixelFormat.Bgra8,
)

const png = encoder.encode(rawBgraBuffer, width, height)
```

## Video encoding

### `VideoEncoder`

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
  WindowsCapture,
} from '@native-capture/windows'

const capture = new WindowsCapture({ monitorIndex: 1 })
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
} from '@native-capture/windows'

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
} from '@native-capture/windows'

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

`Jpeg`, `Png`, `Gif`, `Tiff`, `Bmp`, `JpegXr`

### `ImageEncoderPixelFormat`

`Rgb16F`, `Bgra8`, `Rgba8`

### `DxgiDuplicationFormat`

`Rgba16F`, `Rgb10A2`, `Rgb10XrA2`, `Rgba8`, `Rgba8Srgb`, `Bgra8`, `Bgra8Srgb`

### `VideoCodec`

`Argb32`, `Bgra8`, `D16`, `H263`, `H264`, `H264Es`, `Hevc`, `HevcEs`, `Iyuv`, `L8`, `L16`, `Mjpg`, `Nv12`, `Mpeg1`, `Mpeg2`, `Rgb24`, `Rgb32`, `Wmv3`, `Wvc1`, `Vp9`, `Yuy2`, `Yv12`

### `AudioCodec`

`Aac`, `Ac3`, `AacAdts`, `AacHdcp`, `Ac3Spdif`, `Ac3Hdcp`, `Adts`, `Alac`, `AmrNb`, `AwrWb`, `Dts`, `Eac3`, `Flac`, `Float`, `Mp3`, `Mpeg`, `Opus`, `Pcm`, `Wma8`, `Wma9`, `Vorbis`

### `ContainerFormat`

`Asf`, `Mp3`, `Mpeg4`, `Avi`, `Mpeg2`, `Wave`, `AacAdts`, `Adts`, `ThreeGp`, `Amr`, `Flac`

## Rust compatibility

The binding follows the application-facing portions of [`windows-capture` 2.0.0](https://docs.rs/windows-capture/2.0.0/windows_capture/):

| Rust capability | Node.js API |
| --- | --- |
| Graphics Capture API and `Settings` | `WindowsCapture` and `CaptureOptions` |
| `GraphicsCaptureApiHandler` frame lifecycle | `Frame`, `nextFrame()`, and async iteration |
| Capture thread lifecycle | Promise-based `WindowsCapture.start()`, `nextFrame()`, and `stop()` |
| `Frame` and `FrameBuffer` | `Frame` and packed Node.js `Buffer` data |
| `Monitor` and `Window` | Monitor and window discovery functions |
| `GraphicsCapturePicker` | `usePicker: true` |
| `DxgiDuplicationApi` | `DxgiDuplicationSession` |
| `ImageEncoder` | `ImageEncoder`, `Frame.encode()`, and `Frame.saveAsImage()` |
| `VideoEncoder` | `VideoEncoder`, audio settings, codecs, and containers |
| Graphics Capture feature probes | `isSupported()` and `captureApiSupport()` |

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

Rust formatting uses `cargo fmt`; TypeScript and JavaScript formatting use Oxfmt; linting uses Oxlint. Native builds require the Windows MSVC Rust target.

## Contributing

Contributions are welcome. Native capture changes must be tested on Windows with the MSVC Rust toolchain.

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
- Run the relevant Windows capture checks for platform-specific changes.
- Include a concise description of the behavior changed and the verification performed.

## License

This software is released under the [MIT License](LICENCE).
