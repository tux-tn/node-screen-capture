import { pathToFileURL } from "node:url";

import { enumerateMonitors, enumerateWindows, isSupported } from "@screen-capture/node";

export function discoverSources() {
	if (!isSupported()) {
		throw new Error("Native screen capture is unavailable");
	}
	if (process.platform !== "win32" && process.platform !== "darwin") {
		throw new Error("Wayland does not expose global source discovery; use ScreenCapture with usePicker instead");
	}

	return {
		monitors: enumerateMonitors(),
		windows: enumerateWindows(),
	};
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const sources = discoverSources();
	console.log("Monitors");
	console.table(sources.monitors);
	console.log("Windows");
	console.table(sources.windows);
}
