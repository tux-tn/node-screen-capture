import { performance } from "node:perf_hooks";

import { ColorFormat, ScreenCapture, enumerateMonitors, isSupported } from "../dist/index.mjs";

if (!isSupported()) {
	throw new Error("Native screen capture is unavailable in this session");
}

const isWindows = process.platform === "win32";
if (isWindows) {
	const monitors = enumerateMonitors();
	console.log(`Active screens: ${monitors.length}`);
	console.table(monitors);
	if (monitors.length === 0) throw new Error("No active screens found");
} else {
	console.log("Active screen enumeration is unavailable on Wayland; select a screen in the portal.");
}

const capture = new ScreenCapture({
	...(isWindows ? { monitorIndex: 1 } : { usePicker: true }),
	colorFormat: ColorFormat.Bgra8,
});
const configuredFrameLimit = Number.parseInt(process.env.CAPTURE_MAX_FRAMES ?? "", 10);
const frameLimit = Number.isFinite(configuredFrameLimit) ? configuredFrameLimit : Infinity;
let stopping = false;

async function stop() {
	if (stopping) return;
	stopping = true;
	await capture.stop();
}

for (const signal of ["SIGINT", "SIGTERM"]) {
	process.once(signal, () => {
		void stop();
	});
}

await capture.start();
console.log("Capture started. Press Ctrl+C to stop.");

let totalFrames = 0;
let intervalFrames = 0;
let intervalWaitMs = 0;
let intervalStartedAt = performance.now();

try {
	while (!stopping && totalFrames < frameLimit) {
		const waitStartedAt = performance.now();
		const frame = await capture.nextFrame();
		const waitMs = performance.now() - waitStartedAt;
		if (!frame) break;
		frame.saveAsImage(`frame-${totalFrames}.png`);
		totalFrames += 1;
		intervalFrames += 1;
		intervalWaitMs += waitMs;

		if (totalFrames === 1) {
			console.log(`First frame: ${frame.width}x${frame.height}, ${frame.buffer.byteLength} bytes`);
		}

		const now = performance.now();
		const elapsedMs = now - intervalStartedAt;
		if (elapsedMs >= 1_000) {
			const fps = (intervalFrames * 1_000) / elapsedMs;
			const averageTimeToFrameMs = intervalWaitMs / intervalFrames;
			console.log(`frames=${totalFrames} fps=${fps.toFixed(2)} avgTimeToFrame=${averageTimeToFrameMs.toFixed(2)}ms`);
			intervalFrames = 0;
			intervalWaitMs = 0;
			intervalStartedAt = now;
		}
	}
} finally {
	await stop();
}

console.log(`Capture stopped after ${totalFrames} frames.`);
