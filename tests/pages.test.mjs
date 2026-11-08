import assert from "node:assert/strict";
import test from "node:test";
import { matchesFind, pageCells, pagesForPrinting } from "../src/pages.ts";

const images = ["Cover", "Keep 2", "Keep 3", "Other 4", "Keep 5"].map((name) => ({
  name: `${name}.jpg`,
  path: `C:\\Manual\\${name}.jpg`,
}));
const labels = (cells) => cells.map((cell) => cell.kind === "spacer" ? "_" : cell.page.index + 1);

test("find matches file and folder names case-insensitively", () => {
  assert.equal(matchesFind("Lathe Manual", "lAtHe"), true);
  assert.equal(matchesFind("Lathe Manual", "mill"), false);
  assert.equal(matchesFind("Lathe Manual", ""), true);
});

test("two-page matches retain their original spreads and sides", () => {
  assert.deepEqual(labels(pageCells(images, "", "two-page", true)), ["_", 1, 2, 3, 4, 5]);
  assert.deepEqual(labels(pageCells(images, "keep", "two-page", true)), [2, 3, "_", 5]);
  assert.deepEqual(labels(pageCells(images, "keep", "two-page", false)), ["_", 2, 3, "_", 5, "_"]);
  assert.deepEqual(labels(pageCells(images, "keep", "grid", true)), [2, 3, 5]);
  assert.deepEqual(pageCells(images, "no match", "two-page", true), []);
});

test("printing uses folder order and includes selected pages hidden by find", () => {
  const selected = new Set([images[4].path, images[0].path]);
  assert.deepEqual(pagesForPrinting(images, selected).map((page) => page.index), [0, 4]);
  assert.deepEqual(pagesForPrinting(images, null).map((page) => page.index), [0, 1, 2, 3, 4]);
  assert.deepEqual(pagesForPrinting(images, new Set()), []);
});
