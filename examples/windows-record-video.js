import { pathToFileURL } from "node:url";

import {
	ColorFormat,
	ContainerFormat,
	ScreenCapture,
	VideoCodec,
	VideoEncoder,
	isSupported,
} from "@screen-capture/node";

export async function recordVideo({ outputPath = "capture.mp4", frameLimit = 300, monitorIndex = 1 } = {}) {
	if (process.platform !== "win32") {
		throw new Error("VideoEncoder is available only on Windows");
	}
	if (!isSupported()) {
		throw new Error("Native screen capture is unavailable");
	}
	if (!Number.isSafeInteger(frameLimit) || frameLimit < 1) {
		throw new RangeError("frameLimit must be a positive integer");
	}

	const capture = new ScreenCapture({
		monitorIndex,
		colorFormat: ColorFormat.Bgra8,
	});
	await capture.start();

	let encoder;
	let encodedFrames = 0;
	try {
		const firstFrame = await capture.nextFrame();
		if (!firstFrame) throw new Error("Capture ended before a frame arrived");

		encoder = new VideoEncoder({
			path: outputPath,
			video: {
				width: firstFrame.width,
				height: firstFrame.height,
				codec: VideoCodec.H264,
				bitrate: 15_000_000,
				frameRate: 60,
			},
			container: ContainerFormat.Mpeg4,
		});

		encoder.sendFrame(firstFrame);
		encodedFrames = 1;
		while (encodedFrames < frameLimit) {
			const frame = await capture.nextFrame();
			if (!frame) break;
			encoder.sendFrame(frame);
			encodedFrames += 1;
		}
	} finally {
		try {
			await capture.stop();
		} finally {
			encoder?.finish();
		}
	}

	return { path: outputPath, frames: encodedFrames };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	console.log(
		await recordVideo({
			outputPath: process.argv[2],
			frameLimit: Number.parseInt(process.argv[3] ?? "300", 10),
		}),
	);
}
