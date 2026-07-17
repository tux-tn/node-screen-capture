import { pathToFileURL } from "node:url";

import { ColorFormat, ImageFormat, ScreenCapture, isSupported } from "@screen-capture/node";

export async function captureScreenshot(outputPath = "screenshot.png") {
	if (!isSupported()) {
		throw new Error("Native screen capture is unavailable");
	}

	const capture = new ScreenCapture({ colorFormat: ColorFormat.Bgra8 });
	await capture.start();

	try {
		const frame = await capture.nextFrame();
		if (!frame) throw new Error("Capture ended before a frame arrived");

		frame.saveAsImage(outputPath, ImageFormat.Png);
		return {
			path: outputPath,
			width: frame.width,
			height: frame.height,
		};
	} finally {
		await capture.stop();
	}
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	console.log(await captureScreenshot(process.argv[2]));
}
