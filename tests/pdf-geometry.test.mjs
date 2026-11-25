import test from "node:test";
import assert from "node:assert/strict";
import { normalizedPageRect } from "../desktop/frontend/source/PDF/geometry.ts";

const page = { view: [10, 20, 110, 220] };
const close = (actual, expected) => assert.ok(Math.abs(actual - expected) < 1e-9, `${actual} != ${expected}`);

test("normalizes all four corners against a cropped, unrotated page at a scaled viewport", () => {
  const rect = normalizedPageRect(page, { convertToPdfPoint: (x, y) => [10 + x / 2, 220 - y / 2] }, { x: 20, y: 40, width: 60, height: 80 });
  close(rect.x, .1); close(rect.y, .1); close(rect.width, .3); close(rect.height, .2);
});

test("normalizes a rotated viewport back to the unrotated cropped page coordinates", () => {
  const rect = normalizedPageRect(page, { convertToPdfPoint: (x, y) => [10 + y / 2, 20 + x / 2] }, { x: 40, y: 20, width: 80, height: 60 });
  close(rect.x, .1); close(rect.y, .7); close(rect.width, .3); close(rect.height, .2);
});
