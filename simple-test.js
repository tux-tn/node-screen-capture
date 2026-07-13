import("./dist/index.mjs")
	.then(({ isSupported }) => {
		console.assert(typeof isSupported() === "boolean", "Support probe failed");
		console.info("Windows Capture binding loaded");
	})
	.catch((cause) => {
		console.error(cause);
		process.exitCode = 1;
	});
