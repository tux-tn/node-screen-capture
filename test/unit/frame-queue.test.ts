import { describe, expect, test } from "vitest";

import { LatestFrameQueue } from "../../src/frame-queue.js";

describe("LatestFrameQueue", () => {
	test("resolves a pending consumer with the next frame", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const frame = queue.next();

		queue.push(42);

		await expect(frame).resolves.toBe(42);
	});

	test("keeps only the latest unconsumed frame", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();

		queue.push(1);
		queue.push(2);

		await expect(queue.next()).resolves.toBe(2);
	});

	test("rejects a second pending consumer", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const first = queue.next();

		await expect(queue.next()).rejects.toThrow("nextFrame() already has a pending consumer");

		queue.close();
		await expect(first).resolves.toBeUndefined();
	});

	test("resolves pending and future consumers when closed", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const pending = queue.next();

		queue.close();

		await expect(pending).resolves.toBeUndefined();
		await expect(queue.next()).resolves.toBeUndefined();
	});

	test("ignores frames pushed while closed", async () => {
		const queue = new LatestFrameQueue<number>();

		queue.push(42);

		await expect(queue.next()).resolves.toBeUndefined();
	});

	test("fail rejects a pending consumer", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const pending = queue.next();
		const error = new Error("native capture error");

		queue.fail(error);

		await expect(pending).rejects.toBe(error);
	});

	test("fail rejects future next() calls", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const error = new Error("native capture error");

		queue.fail(error);

		await expect(queue.next()).rejects.toBe(error);
	});

	test("open resets failure state", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const error = new Error("native capture error");

		queue.fail(error);
		queue.open();

		const frame = queue.next();
		queue.push(42);
		await expect(frame).resolves.toBe(42);
	});

	test("close still resolves undefined when no failure is set", async () => {
		const queue = new LatestFrameQueue<number>();
		queue.open();
		const pending = queue.next();

		queue.close();

		await expect(pending).resolves.toBeUndefined();
		await expect(queue.next()).resolves.toBeUndefined();
	});
});
