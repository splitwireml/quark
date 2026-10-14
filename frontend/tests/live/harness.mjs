// Starts the Rust dev server, Vite and a browser so a pass can drive the real App against the real core.
// Run directly, it runs every `*-browser.mjs` pass here, or those whose name contains an argument.
import { spawn } from "node:child_process";
import { mkdtempSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createServer } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));
const frontend = path.resolve(here, "../..");
const binary = path.resolve(frontend, "../target/debug/quark-dev-server" + (process.platform === "win32" ? ".exe" : ""));
const engines = new Set(["webkit", "chromium"]);

async function startCore(data) {
  const child = spawn(binary, ["--data-dir", data, "--cache-dir", path.join(data, "cache"), "--port", "0"], { stdio: ["ignore", "pipe", "inherit"], env: { ...process.env, RUST_LOG: "warn" } });
  const port = await new Promise((resolve, reject) => {
    let seen = "";
    const timer = setTimeout(() => reject(new Error("quark-dev-server did not report QUARK_LISTENING")), 30000);
    child.once("error", reject);
    child.once("exit", code => reject(new Error(`quark-dev-server exited early with ${code}`)));
    child.stdout.on("data", chunk => {
      seen += chunk;
      const match = seen.match(/QUARK_LISTENING \S+:(\d+)/);
      if (match) { clearTimeout(timer); resolve(Number(match[1])); }
    });
  });
  return { port, stop: () => { child.removeAllListeners("exit"); child.kill(); } };
}

async function startVite(port) {
  const server = await createServer({
    root: frontend,
    logLevel: "error",
    server: {
      host: "127.0.0.1",
      port: 0,
      proxy: { "/api": `http://127.0.0.1:${port}` },
      fs: { allow: [frontend, realpathSync(path.join(frontend, "node_modules"))] }
    }
  });
  await server.listen();
  return { url: server.resolvedUrls.local[0].replace(/\/$/, ""), stop: () => server.close() };
}

async function launch() {
  const name = process.env.QUARK_LIVE_BROWSER || "webkit";
  if (!engines.has(name)) throw new Error(`QUARK_LIVE_BROWSER must be webkit or chromium, not ${name}`);
  const playwright = await import(process.env.PLAYWRIGHT_MODULE || "playwright");
  return (playwright[name] ?? playwright.default[name]).launch({ headless: true });
}

export function createApi(base) {
  const call = async (url, init) => {
    const response = await fetch(base + url, init);
    if (!response.ok) throw new Error(`${init?.method ?? "GET"} ${url} -> ${response.status} ${await response.text()}`);
    return response.json();
  };
  const post = (url, body) => call(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const upload = (url, name, text) => {
    const body = new FormData();
    body.append("file", new Blob([text], { type: "text/csv" }), name);
    return call(url, { method: "POST", body });
  };
  return { get: call, post, upload };
}

// Runs `pass({ page, api, project, node, url })` with everything torn down afterwards, even on failure.
export async function withLive(fixture, pass) {
  const data = mkdtempSync(path.join(tmpdir(), "quark-live-"));
  const stops = [];
  try {
    const core = await startCore(data); stops.push(core.stop);
    const vite = await startVite(core.port); stops.push(vite.stop);
    const browser = await launch(); stops.push(() => browser.close());
    const api = createApi(`http://127.0.0.1:${core.port}`);
    const project = await api.post("/api/projects", { name: "Live" });
    const node = await api.upload(`/api/projects/${project.id}/sources/upload`, fixture.name, fixture.csv);
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    page.setDefaultTimeout(10000);
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await pass({ page, api, project, node, url: vite.url });
    if (errors.length) throw new Error(`Page errors: ${errors.join("; ")}`);
  } finally {
    for (const stop of stops.reverse()) await Promise.resolve(stop()).catch(() => {});
    rmSync(data, { recursive: true, force: true });
  }
}

// Runs each named step in order and fails loudly with the step's name.
export async function runSteps(steps) {
  for (const [name, step] of Object.entries(steps)) {
    try { await step(); } catch (error) { throw new Error(`${name}: ${error.message}`, { cause: error }); }
    console.log(`ok ${name}`);
  }
}

async function main() {
  const filters = process.argv.slice(2);
  const passes = readdirSync(here).filter(file => file.endsWith("-browser.mjs") && (!filters.length || filters.some(filter => file.includes(filter)))).sort();
  if (!passes.length) throw new Error(`No live pass matches ${filters.join(", ") || "anything"}`);
  for (const file of passes) {
    console.log(`# ${file}`);
    const code = await new Promise(resolve => spawn(process.execPath, [path.join(here, file)], { stdio: "inherit" }).once("exit", resolve));
    if (code !== 0) process.exit(code || 1);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => { console.error(error.message); process.exit(1); });
}
