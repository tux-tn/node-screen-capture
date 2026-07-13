export class LatestFrameQueue<T> {
	#latest: T | undefined;
	#pending:
		| {
				resolve: (value: T | undefined) => void;
				promise: Promise<T | undefined>;
		  }
		| undefined;
	#closed = true;

	open(): void {
		this.#closed = false;
		this.#latest = undefined;
	}

	push(value: T): void {
		if (this.#closed) return;
		if (this.#pending) {
			const { resolve } = this.#pending;
			this.#pending = undefined;
			resolve(value);
		} else {
			this.#latest = value;
		}
	}

	next(): Promise<T | undefined> {
		if (this.#latest !== undefined) {
			const value = this.#latest;
			this.#latest = undefined;
			return Promise.resolve(value);
		}
		if (this.#closed) return Promise.resolve(undefined);
		if (this.#pending) {
			return Promise.reject(new Error("nextFrame() already has a pending consumer"));
		}
		const { promise, resolve } = Promise.withResolvers<T | undefined>();
		this.#pending = { promise, resolve };
		return promise;
	}

	close(): void {
		this.#closed = true;
		this.#latest = undefined;
		if (this.#pending) {
			const { resolve } = this.#pending;
			this.#pending = undefined;
			resolve(undefined);
		}
	}
}
