import { defineConfig } from "tsdown";

export default defineConfig([
	{
		entry: "src/index.ts",
		name: "windows-capture",
		shims: true,
		sourcemap: true,
	},
]);
