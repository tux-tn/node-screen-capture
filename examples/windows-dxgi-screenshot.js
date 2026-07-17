import { pathToFileURL } from "node:url";

import { DxgiDuplicationFormat, DxgiDuplicationSession, ImageFormat } from "@screen-capture/node";

export function captureDxgiScreenshot(outputPath = "dxgi-screenshot.png") {
	if (process.platform !== "win32") {
		throw new Error("DXGI Desktop Duplication is available only on Windows");
	}

	const session = new DxgiDuplicationSession({
		monitorIndex: 1,
		supportedFormats: [DxgiDuplicationFormat.Bgra8, DxgiDuplicationFormat.Rgba8],
	});
	const frame = session.acquireNextFrame(1_000);
	if (!frame) throw new Error("No desktop update arrived before the timeout");

	frame.saveAsImage(outputPath, ImageFormat.Png);
	return {
		path: outputPath,
		width: frame.width,
		height: frame.height,
		format: frame.colorFormat,
	};
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	console.log(captureDxgiScreenshot(process.argv[2]));
}
