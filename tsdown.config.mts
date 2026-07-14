import { defineConfig } from "tsdown";

export default defineConfig([
	{
		entry: "src/index.ts",
		deps: { neverBundle: ["../index.js"] },
		shims: true,
		sourcemap: true,
	},
]);
