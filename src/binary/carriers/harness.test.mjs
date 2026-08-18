// Covers the two halves most likely to drift from the worker wrapper
// archival-editor's deploy server generates: how a request body is parsed, and
// how a carrier's return value becomes a response.
//
// Run with: node --test src/binary/carriers/harness.test.mjs

import assert from "node:assert/strict";
import test from "node:test";
import { parseRequestBody, toResponse } from "./harness.mjs";

test("only POST and PUT carry a body", async () => {
  for (const method of ["GET", "HEAD", "DELETE", "OPTIONS"]) {
    assert.equal(
      await parseRequestBody(method, "application/json", '{"a":1}'),
      null,
    );
  }
  assert.deepEqual(await parseRequestBody("POST", "application/json", '{"a":1}'), {
    a: 1,
  });
  assert.deepEqual(await parseRequestBody("PUT", "application/json", '{"a":1}'), {
    a: 1,
  });
});

test("form bodies arrive as a flat object", async () => {
  assert.deepEqual(
    await parseRequestBody(
      "POST",
      "application/x-www-form-urlencoded",
      "name=Tormenta&genre=rock",
    ),
    { name: "Tormenta", genre: "rock" },
  );
});

test("multipart bodies arrive as a flat object", async () => {
  const form = new FormData();
  form.set("name", "Tormenta");
  const request = new Request("http://carrier.local", {
    method: "POST",
    body: form,
  });
  const body = Buffer.from(await request.arrayBuffer());
  assert.deepEqual(
    await parseRequestBody("POST", request.headers.get("content-type"), body),
    { name: "Tormenta" },
  );
});

test("an unrecognized content type is delivered as text", async () => {
  assert.equal(
    await parseRequestBody("POST", "text/plain", "just words"),
    "just words",
  );
  assert.equal(await parseRequestBody("POST", undefined, "no type"), "no type");
});

test("a charset does not stop a json body being parsed", async () => {
  assert.deepEqual(
    await parseRequestBody("POST", "application/json; charset=utf-8", '{"a":1}'),
    { a: 1 },
  );
});

test("an object is sent as json", async () => {
  const response = toResponse({ hello: "world" });
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-type"), "application/json");
  assert.deepEqual(await response.json(), { hello: "world" });
});

test("an array is sent as json", async () => {
  const response = toResponse([1, 2]);
  assert.equal(response.headers.get("content-type"), "application/json");
  assert.deepEqual(await response.json(), [1, 2]);
});

test("a string is sent as text", async () => {
  const response = toResponse("hi");
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get("content-type"),
    "text/plain; charset=utf-8",
  );
  assert.equal(await response.text(), "hi");
});

test("a redirect: prefix becomes a 302", async () => {
  const response = toResponse("redirect:/login");
  assert.equal(response.status, 302);
  assert.equal(response.headers.get("location"), "/login");
  assert.equal(await response.text(), "/login");
});

test("a Response is passed through untouched", () => {
  const original = new Response("body", { status: 418 });
  assert.equal(toResponse(original), original);
});

test("anything else is a 500", async () => {
  for (const value of [undefined, 42, true]) {
    assert.equal(toResponse(value).status, 500);
  }
  assert.match(
    await toResponse(42).text(),
    /^Invalid response from carrier: 42$/,
  );
});

test("null is json, matching typeof null === object", async () => {
  // Deliberately the same quirk the deployed wrapper has.
  const response = toResponse(null);
  assert.equal(response.status, 200);
  assert.equal(await response.text(), "null");
});
