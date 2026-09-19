// Isolated full-frame replay: no Winit event loop or interactive editor launch.
const { spawn } = require("node:child_process");
const { once } = require("node:events");
const fs = require("node:fs");
const { chromium } = require("playwright");

// Playwright exposes page CDP sessions, but not worker sessions. Route the
// worker's diagnostic commands through Chrome's Target protocol instead.
async function workerProfiler(browser) {
  const connection = await browser.newBrowserCDPSession();
  const { targetInfos } = await connection.send("Target.getTargets");
  const target = targetInfos.find(t => t.type === "worker" && t.url.endsWith("/worker.js"));
  if (!target) throw new Error("Computation coordinator not found");
  const { sessionId } = await connection.send("Target.attachToTarget", { targetId: target.targetId, flatten: false });
  let nextId = 0;
  const pending = new Map();
  connection.on("Target.receivedMessageFromTarget", event => {
    if (event.sessionId !== sessionId) return;
    const reply = JSON.parse(event.message);
    const request = pending.get(reply.id);
    if (!request) return;
    pending.delete(reply.id);
    clearTimeout(request.timer);
    if (reply.error) request.reject(new Error(JSON.stringify(reply.error)));
    else request.resolve(reply.result);
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Worker ${method} timed out`)); }, 30000);
    pending.set(id, { resolve, reject, timer });
    connection.send("Target.sendMessageToTarget", { sessionId, message: JSON.stringify({ id, method, params }) })
      .catch(error => { pending.delete(id); clearTimeout(timer); reject(error); });
  });
  await send("Profiler.enable");
  await send("Profiler.setSamplingInterval", { interval: 500 });
  return { send, close: () => connection.detach() };
}

(async () => {
  const query = process.argv[2] ?? "position=0.5";
  const label = process.argv[3] ?? "baseline";
  const server = spawn("python3", ["-B", "website/preview.py", "--no-open"], { stdio: ["ignore", "pipe", "ignore"] });
  let browser;
  try {
    const url = await new Promise((resolve, reject) => {
      let output = "";
      server.stdout.on("data", chunk => {
        output += chunk;
        const match = output.match(/Progred website: (http:\/\/\S+)/);
        if (match) resolve(match[1]);
      });
      server.once("error", reject);
      server.once("exit", code => reject(new Error(`Server exited: ${code}`)));
    });
    browser = await chromium.launch({ channel: "chrome", headless: true, chromiumSandbox: true });
    const page = await browser.newPage({ viewport: { width: 1240, height: 960 } });
    await page.addInitScript(() => {
      window.meshWrites = 0;
      if (typeof GPUQueue !== "undefined") {
        const write = GPUQueue.prototype.writeBuffer;
        GPUQueue.prototype.writeBuffer = function(...args) {
          if (args[0].label === "mesh stream") window.meshWrites++;
          return write.apply(this, args);
        };
      }
      // Diagnostic control: hold new jobs only after initial rendering settles.
      // The real transport and latest-request slot remain unchanged. Resume
      // delivers each captured message once before this isolated page closes.
      window.holdJobs = false;
      const held = [];
      window.heldJobCount = () => held.length;
      const post = Worker.prototype.postMessage;
      Worker.prototype.postMessage = function(...args) {
        if (window.holdJobs && args[0]?.type === "job") held.push([this, args]);
        else return post.apply(this, args);
      };
      window.resumeJobs = () => {
        window.holdJobs = false;
        for (const [worker, args] of held.splice(0)) post.apply(worker, args);
      };
    });
    page.on("pageerror", error => console.error(error));
    await page.goto(`${url}editor/orbit-profile.html?${query}`);
    await page.waitForFunction(() => window.runOrbit || window.profileError, null, { timeout: 240000 });
    const setup = await page.evaluate(() => ({ error: window.profileError, backend: window.backend, memoryBefore: window.profileMemory?.() }));
    if (setup.error) throw new Error(setup.error);
    if (new URLSearchParams(query).has("hold-jobs")) {
      await page.evaluate(() => { window.holdJobs = true; });
    }
    await page.evaluate(() => window.runOrbit(8));
    await page.evaluate(() => { window.meshWrites = 0; });
    const samples = await page.evaluate(() => window.runOrbit(80));
    const counts = await page.evaluate(() => ({ meshWrites: window.meshWrites, heldJobs: window.heldJobCount(), memoryAfter: window.profileMemory() }));
    const summary = Object.fromEntries(Object.keys(samples[0]).map(key => {
      const values = samples.map(s => s[key]).sort((a,b) => a-b);
      return [key, { mean: values.reduce((sum, value) => sum + value, 0) / values.length,
        median: values[40], p95: values[76], max: values[79] }];
    }));
    const metadata = { query, browser: browser.version(), ...setup, ...counts };
    console.log(JSON.stringify({ ...metadata, summary }));
    fs.writeFileSync(`target/orbit-${label}.json`, JSON.stringify({ ...metadata, samples, summary }));
    await page.screenshot({ path: `target/orbit-${label}.png` });
    const session = await page.context().newCDPSession(page);
    await session.send("Profiler.enable");
    await session.send("Profiler.setSamplingInterval", { interval: 500 });
    const worker = new URLSearchParams(query).has("profile-worker") ? await workerProfiler(browser) : null;
    if (worker) await worker.send("Profiler.start");
    await session.send("Profiler.start");
    await page.evaluate(() => window.runOrbit(40));
    const { profile } = await session.send("Profiler.stop");
    fs.writeFileSync(`target/orbit-${label}.cpuprofile`, JSON.stringify(profile));
    console.log(`Saved CPU profile: target/orbit-${label}.cpuprofile`);
    if (worker) {
      const { profile } = await worker.send("Profiler.stop");
      fs.writeFileSync(`target/orbit-${label}-worker.cpuprofile`, JSON.stringify(profile));
      await worker.close();
      console.log(`Saved worker CPU profile: target/orbit-${label}-worker.cpuprofile`);
    }
    await page.evaluate(() => window.resumeJobs());
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null && server.signalCode === null) {
      const stopped = once(server, "exit"); server.kill("SIGTERM"); await stopped;
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
