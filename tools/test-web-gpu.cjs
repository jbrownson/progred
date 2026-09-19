// Pixel checks of the actual browser presentation backend. No editor startup.
const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { once } = require("node:events");
const { chromium } = require("playwright");

(async () => {
  const server = spawn("python3", ["-B", "website/preview.py", "--no-open"], {
    stdio: ["ignore", "pipe", "inherit"],
  });
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
    for (const fallback of [false, true]) {
      const page = await browser.newPage();
      const errors = [];
      page.on("pageerror", error => errors.push(String(error)));
      page.on("console", message => {
        if (message.type() === "error") errors.push(message.text());
      });
      if (fallback) await page.addInitScript(() => Object.defineProperty(navigator, "gpu", { value: undefined }));
      await page.goto(`${url}editor/gpu-probe.html`);
      await page.waitForFunction(() => window.drawProbe || window.probeError, null, { timeout: 60000 });
      const setup = await page.evaluate(() => ({ error: window.probeError, backend: window.backend }));
      assert.equal(setup.error, undefined);
      assert.equal(setup.backend, fallback ? "Canvas2D" : "WebGPU");
      for (const [stage, width, height] of [[0, 220, 100], [1, 240, 110], [0, 220, 100]]) {
        const bytes = await page.evaluate(async args => window.drawProbe(...args), [stage, width, height]);
        const at = (x, y) => bytes.slice((y * width + x) * 4, (y * width + x) * 4 + 4);
        assert.deepEqual(at(2, 2), [0, 255, 0, 255], "vector below");
        assert.deepEqual(at(60, 50), [0, 255, 0, 255], "rect clip excludes mesh");
        assert.deepEqual(at(30, 35), [0, 0, 0, 255], "vector above mesh");
        const first = at(37, 45);
        assert.ok(first[stage === 0 ? 0 : 2] > 150 && first[stage === 0 ? 2 : 0] < 3, `first mesh: ${first}`);
        assert.ok(at(110, 35)[2] > 150 && at(110, 35)[0] < 3, "second mesh");
        assert.deepEqual(at(83, 55), [255, 255, 255, 255], "curved clip excludes mesh");
        assert.deepEqual(at(165, 40), [0, 255, 255, 255], "completed implicit pixels occlude triangles");
        assert.ok(at(185, 40)[0] > 150 && at(185, 40)[1] < 3, "unknown implicit pixels retain mesh");
        assert.deepEqual(at(84, 80), [255, 128, 0, 255], "CPU image uploads in order");
        assert.deepEqual(at(width - 1, height - 1), [255, 255, 255, 255], "resized output cleared");
      }
      assert.deepEqual(errors.filter(e => !e.includes("404")), [], "no browser/GPU validation errors");
      console.log(`PASS: ${setup.backend} ordering, two previews, rectangular/curved clips, partial depth, image upload, resize, geometry replacement`);
      await page.close();
    }
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null && server.signalCode === null) {
      const stopped = once(server, "exit"); server.kill("SIGTERM"); await stopped;
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
