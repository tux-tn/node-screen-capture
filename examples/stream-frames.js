import { pathToFileURL } from "node:url";

import { ColorFormat, ScreenCapture, isSupported } from "@screen-capture/node";

export async function streamFrames(onFrame, frameLimit = 120) {
	if (!isSupported()) {
		throw new Error("Native screen capture is unavailable");
	}
	if (!Number.isSafeInteger(frameLimit) || frameLimit < 1) {
		throw new RangeError("frameLimit must be a positive integer");
	}

	const capture = new ScreenCapture({ colorFormat: ColorFormat.Bgra8 });
	let frameNumber = 0;

	for await (const frame of capture) {
		await onFrame(frame, frameNumber);
		frameNumber += 1;
		if (frameNumber >= frameLimit) break;
	}

	return frameNumber;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const captured = await streamFrames(
		(frame, frameNumber) => {
			console.log(`frame=${frameNumber} size=${frame.width}x${frame.height} bytes=${frame.buffer.byteLength}`);
		},
		Number.parseInt(process.argv[2] ?? "120", 10),
	);

	console.log(`Captured ${captured} frames`);
}
