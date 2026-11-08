type ThumbnailJob = {
  priority: () => number;
  load: () => Promise<void>;
  onError: (reason: unknown) => void;
  state: "pending" | "running" | "done" | "cancelled";
};

export class ThumbnailQueue {
  private active = 0;
  private pending: ThumbnailJob[] = [];
  private readonly concurrency: number;

  constructor(concurrency: number) {
    if (!Number.isInteger(concurrency) || concurrency < 1) {
      throw new Error("Thumbnail concurrency must be a positive integer.");
    }
    this.concurrency = concurrency;
  }

  enqueue(priority: () => number, load: () => Promise<void>, onError: (reason: unknown) => void): () => boolean {
    const job: ThumbnailJob = { priority, load, onError, state: "pending" };
    this.pending.push(job);
    queueMicrotask(() => this.drain());
    return () => {
      if (job.state !== "pending") return false;
      job.state = "cancelled";
      this.pending = this.pending.filter((item) => item !== job);
      return true;
    };
  }

  private drain() {
    while (this.active < this.concurrency && this.pending.length > 0) {
      this.pending.sort((a, b) => a.priority() - b.priority());
      const job = this.pending.shift()!;
      job.state = "running";
      this.active++;
      void Promise.resolve()
        .then(job.load)
        .catch(job.onError)
        .finally(() => {
          job.state = "done";
          this.active--;
          this.drain();
        });
    }
  }
}

export const thumbnailQueue = new ThumbnailQueue(3);
