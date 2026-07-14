import { isSupported } from "../dist/index.mjs";

try {
	console.assert(typeof isSupported() === "boolean", "Support probe failed");
	console.log("Support probe succeeded");
} catch (cause) {
	console.error(cause);
	process.exitCode = 1;
}
