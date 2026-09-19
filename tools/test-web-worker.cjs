// Headless threading diagnostics only; never starts the Progred editor.
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
      server.stdout.on("data", (chunk) => {
        output += chunk;
        const match = output.match(/Progred website: (http:\/\/\S+)/);
        if (match) resolve(match[1]);
      });
      server.once("error", reject);
      server.once("exit", (code) => reject(new Error(`Preview server exited: ${code}`)));
    });
    browser = await chromium.launch({ channel: "chrome", headless: true, chromiumSandbox: true });
    let failed = false;
    for (const query of ["?startup", "", "?replacement&trial=1", "?replacement&trial=2", "?replacement&trial=3", "?fidget", "?fidget&cancel"]) {
      const page = await browser.newPage();
      page.on("pageerror", (error) => console.error(error));
      await page.goto(`${url}editor/worker-probe.html${query}`);
      await page.waitForFunction(() => /^(PASS|FAIL):/.test(document.querySelector("#result").textContent), null, { timeout: 30000 });
      const result = await page.locator("#result").textContent();
      console.log(JSON.stringify({ query, result }));
      failed ||= result.startsWith("FAIL:");
      await page.close();
    }
    if (failed) process.exitCode = 1;
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null && server.signalCode === null) {
      const stopped = once(server, "exit");
      server.kill("SIGTERM");
      await stopped;
    }
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
