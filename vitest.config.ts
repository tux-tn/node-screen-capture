import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		environment: "node",
		include: ["test/**/*.test.ts"],
		clearMocks: true,
		restoreMocks: true,
		slowTestThreshold: 3000,
		pool: "threads",
		reporters: [["verbose", { summary: true }]],
		coverage: {
			provider: "v8",
			reporter: ["html", "text-summary", "lcov"],
			include: ["src/**/*.ts"],
			reportsDirectory: "./coverage",
		},
	},
});
