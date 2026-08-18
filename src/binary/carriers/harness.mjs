// Hosts a site's carriers for `archival run`. The dev server spawns this and
// reverse-proxies /carriers/* to it.
//
// The request/response half of this file must behave identically to the worker
// wrapper archival-editor's deploy server generates
// (api/carriers/deploy-server/deploy-carrier.mjs) - that is what makes a
// carrier that works locally work once deployed.
//
// Imports nothing but node builtins: it lives outside the site, so a bare
// specifier here would resolve against the wrong node_modules. A carrier's own
// imports are unaffected, resolving from the carrier's directory as usual.

import http from "node:http";
import { realpathSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";

const TOKEN = process.env.ARCHIVAL_CARRIER_TOKEN || "";
const PORT = Number(process.env.ARCHIVAL_CARRIER_PORT || 0);

const SHA_RE = /^[a-f0-9]{64}$/i;
const CARRIER_ROUTE = /^\/carrier\/([A-Za-z0-9_][A-Za-z0-9_.-]*)(?:\/.*)?$/;

/** The most recent payload pushed by the dev server. */
let state = null;
/** entry path -> import promise, so a carrier is only evaluated once. */
const modules = new Map();

const deepFreeze = (value) => {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) {
      deepFreeze(child);
    }
  }
  return value;
};

const uploadKey = (prefix, sha, filename) => {
  if (typeof sha !== "string" || !SHA_RE.test(sha)) {
    throw new Error("Invalid upload sha: " + sha);
  }
  if (typeof filename !== "string" || !filename) {
    throw new Error("An upload filename is required");
  }
  // The prefix is what scopes reads to this site, so nothing a carrier passes
  // may walk out of it.
  if (filename.startsWith("/") || filename.split("/").includes("..")) {
    throw new Error("Invalid upload filename: " + filename);
  }
  return (
    prefix + sha + "/" + filename.split("/").map(encodeURIComponent).join("/")
  );
};

/**
 * Builds objects.UPLOADS. Called per request so the filename index is memoized
 * for one request without carrying stale uploads into the next.
 */
const makeUploads = () => {
  const prefix = state.uploadPrefix;
  if (!prefix) {
    const unavailable = () => {
      throw new Error(
        "objects.UPLOADS is not available for this deploy (no upload prefix)",
      );
    };
    return { list: unavailable, get: unavailable };
  }
  // Locally there is no bucket to enumerate, so the dev server derives this
  // from the files the site's objects point at.
  const list = async () => state.uploads;
  const get = async (file, sha) => {
    let filename = file;
    let fileSha = sha;
    // A file value off objects already carries both halves.
    if (file && typeof file === "object") {
      filename = file.filename;
      fileSha = sha || file.sha;
    }
    if (typeof filename !== "string" || !filename) {
      throw new Error("UPLOADS.get needs a filename or a file object");
    }
    if (!fileSha) {
      const matches = (await list()).filter((e) => e.filename === filename);
      if (matches.length === 0) {
        return null;
      }
      if (matches.length > 1) {
        throw new Error(
          "More than one upload is named " +
            filename +
            " - pass a sha to say which one: " +
            matches.map((m) => m.sha).join(", "),
        );
      }
      fileSha = matches[0].sha;
    }
    const key = uploadKey(prefix, fileSha, filename);
    const url = state.uploadsUrl.replace(/\/$/, "") + "/" + key;
    const response = await fetch(url);
    if (response.status === 404) {
      return null;
    }
    if (!response.ok) {
      throw new Error(
        "Failed reading upload " + key + ": " + response.status + " from " + url,
      );
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    const etag = (response.headers.get("etag") || "").replace(/^"|"$/g, "");
    return {
      key,
      size: bytes.byteLength,
      etag,
      httpEtag: etag ? '"' + etag + '"' : "",
      get body() {
        return new Blob([bytes]).stream();
      },
      arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
      text: async () => new TextDecoder().decode(bytes),
      json: async () => JSON.parse(new TextDecoder().decode(bytes)),
      blob: async () => new Blob([bytes]),
      writeHttpMetadata: (headers) => {
        const type = response.headers.get("content-type");
        const encoding = response.headers.get("content-encoding");
        if (type) headers.set("content-type", type);
        if (encoding) headers.set("content-encoding", encoding);
      },
    };
  };
  return { list, get };
};

const readBody = (req) =>
  new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });

/**
 * Only POST and PUT carry a body. A Response is used as the parser so that
 * multipart and urlencoded bodies are decoded by the platform, exactly as they
 * are in a deployed carrier.
 */
export const parseRequestBody = async (method, contentType, body) => {
  if (method !== "POST" && method !== "PUT") {
    return null;
  }
  const type = (contentType || "").toLowerCase();
  const as = () => new Response(body, { headers: { "content-type": type } });
  if (type.indexOf("application/json") !== -1) {
    return as().json();
  }
  if (
    type.indexOf("application/x-www-form-urlencoded") !== -1 ||
    type.indexOf("multipart/form-data") !== -1 ||
    type.indexOf("application/form") !== -1
  ) {
    const formData = await as().formData();
    const obj = {};
    for (const [key, value] of formData.entries()) {
      obj[key] = value;
    }
    return obj;
  }
  // Fallback: treat everything else as text
  return as().text();
};

/** Maps whatever a carrier returned onto a Response. */
export const toResponse = (result) => {
  if (result instanceof Response) {
    return result;
  }
  switch (typeof result) {
    case "object":
      return new Response(JSON.stringify(result), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    case "string": {
      const isRedirect = result.startsWith("redirect:");
      const status = isRedirect ? 302 : 200;
      const extraHeaders = {};
      let body = result;
      if (isRedirect) {
        body = result.slice(9);
        extraHeaders["Location"] = body;
      }
      return new Response(body, {
        status,
        headers: {
          "content-type": "text/plain; charset=utf-8",
          ...extraHeaders,
        },
      });
    }
    default:
      return new Response("Invalid response from carrier: " + result, {
        status: 500,
      });
  }
};

const loadCarrier = (entry) => {
  let loading = modules.get(entry);
  if (!loading) {
    // Failures are memoized too: a carrier that throws at import time should
    // report the same error on every request, not race a half-loaded module.
    loading = import(pathToFileURL(entry).href);
    modules.set(entry, loading);
  }
  return loading;
};

const runCarrier = async (name, req, body) => {
  const entry = state.carriers[name];
  if (!entry) {
    return new Response("No carrier named " + name, { status: 404 });
  }
  const params = new URL(req.url, "http://carrier.local").searchParams;
  try {
    const module = await loadCarrier(entry);
    const carrier = module.default;
    if (typeof carrier !== "function") {
      return new Response(
        "Carrier " + name + " does not default-export a function",
        { status: 500 },
      );
    }
    const parsed = await parseRequestBody(
      req.method,
      req.headers["content-type"],
      body,
    );
    // UPLOADS has methods, so unlike the rest of the vars it cannot be part of
    // the pushed payload. It wins over a site object of the same name, which is
    // how every injected var resolves that collision.
    const objects = Object.freeze({
      ...state.objects,
      SITE_URL: state.siteUrl,
      UPLOADS: makeUploads(),
    });
    return toResponse(await carrier(params, parsed, objects));
  } catch (e) {
    return new Response("Carrier threw an error: " + e, { status: 500 });
  }
};

const send = async (res, response) => {
  res.statusCode = response.status;
  for (const [key, value] of response.headers.entries()) {
    if (key.toLowerCase() === "set-cookie") continue;
    res.setHeader(key, value);
  }
  const cookies = response.headers.getSetCookie?.() ?? [];
  if (cookies.length) {
    res.setHeader("set-cookie", cookies);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  res.end(bytes);
};

const authorized = (req) => TOKEN && req.headers["x-archival-token"] === TOKEN;

const server = http.createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://carrier.local").pathname;
    if (path.startsWith("/__control/")) {
      if (!authorized(req)) {
        return send(res, new Response("Forbidden", { status: 403 }));
      }
      if (path === "/__control/health") {
        return send(
          res,
          Response.json({
            ok: true,
            carriers: state ? Object.keys(state.carriers) : [],
            hasState: !!state,
          }),
        );
      }
      if (path === "/__control/state" && req.method === "POST") {
        const pushed = JSON.parse((await readBody(req)).toString("utf-8"));
        // Carriers are re-imported on restart, not on a state push, so the
        // module cache is deliberately left alone here.
        state = { ...pushed, objects: deepFreeze(pushed.objects) };
        return send(res, Response.json({ ok: true }));
      }
      return send(res, new Response("Not Found", { status: 404 }));
    }
    const route = CARRIER_ROUTE.exec(path);
    if (!route) {
      return send(res, new Response("Not Found", { status: 404 }));
    }
    if (!state) {
      return send(res, new Response("Carriers are still starting", { status: 503 }));
    }
    const body = await readBody(req);
    return send(res, await runCarrier(route[1], req, body));
  } catch (e) {
    console.error("[carrier] request failed:", e);
    if (!res.headersSent) {
      res.statusCode = 500;
    }
    res.end("Carrier harness failed: " + e);
  }
});

export const start = () => {
  // One carrier's unhandled failure must not take down the others.
  process.on("uncaughtException", (e) => console.error("[carrier]", e));
  process.on("unhandledRejection", (e) => console.error("[carrier]", e));

  // The dev server holds this pipe open and never writes to it, so this fires
  // even when the parent dies in a way that runs no cleanup.
  process.stdin.on("end", () => process.exit(0));
  process.stdin.on("close", () => process.exit(0));
  process.stdin.resume();

  server.listen(PORT, "127.0.0.1", () => {
    // The dev server reads the port from here rather than picking one itself,
    // so there is no window in which the port is taken by something else.
    console.log(
      JSON.stringify({ archivalCarrier: { port: server.address().port } }),
    );
  });
};

// Both sides are resolved before comparing: node resolves symlinks when it
// loads a module, but argv[1] is whatever the caller passed, and the harness
// lives under a temp directory that is a symlink on macOS.
const isEntrypoint = () => {
  if (!process.argv[1]) return false;
  try {
    return (
      realpathSync(fileURLToPath(import.meta.url)) ===
      realpathSync(process.argv[1])
    );
  } catch {
    return false;
  }
};

// Importing this file (the tests do) must not bind a port or hold the process
// open, so nothing starts until it is run as the entrypoint.
if (isEntrypoint()) {
  start();
}
