import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import test from "node:test";
import { ThumbnailQueue } from "../src/thumbnailQueue.ts";

test("limits concurrent loads and prioritises the nearest pending page", async () => {
  const queue = new ThumbnailQueue(2);
  const started = [];
  const finish = [];
  const load = (name) => () => new Promise((resolve) => {
    started.push(name);
    finish.push(resolve);
  });
  let farDistance = 400;
  queue.enqueue(() => 0, load("first"), assert.fail);
  queue.enqueue(() => 0, load("second"), assert.fail);
  queue.enqueue(() => farDistance, load("far"), assert.fail);
  queue.enqueue(() => 100, load("near"), assert.fail);
  await setImmediate();
  assert.deepEqual(started, ["first", "second"]);
  farDistance = 50;
  finish.shift()();
  await setImmediate();
  assert.deepEqual(started, ["first", "second", "far"]);
  finish.shift()();
  await setImmediate();
  assert.deepEqual(started, ["first", "second", "far", "near"]);
  finish.forEach((resolve) => resolve());
});

test("cancels pending work and reports load failures", async () => {
  const queue = new ThumbnailQueue(1);
  let finish;
  const errors = [];
  const started = [];
  queue.enqueue(() => 0, () => new Promise((resolve) => { finish = resolve; }), assert.fail);
  const cancel = queue.enqueue(() => 1, async () => { started.push("cancelled"); }, assert.fail);
  queue.enqueue(() => 2, async () => { throw new Error("decode failed"); }, (error) => errors.push(error.message));
  await setImmediate();
  assert.equal(cancel(), true);
  assert.equal(cancel(), false);
  finish();
  await setImmediate();
  assert.deepEqual(started, []);
  assert.deepEqual(errors, ["decode failed"]);
});
