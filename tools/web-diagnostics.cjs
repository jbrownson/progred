// The preview server and headless Chrome that the browser diagnostics run
// against. website/preview.py builds editor changes before serving.
const { spawn } = require("node:child_process");
const { once } = require("node:events");
// tools/package.json pins playwright-core; it drives the installed Chrome.
const { chromium } = require("playwright-core");

// Calls `use(url, browser)` with a fresh server and browser, and closes both
// afterwards. `serverErrors` is the server's stderr handling.
async function withPreview(use, { serverErrors = "inherit" } = {}) {
  const server = spawn("python3", ["-B", "website/preview.py", "--no-open", "--port", "0"], {
    stdio: ["ignore", "pipe", serverErrors],
  });
  let browser;
  try {
    const url = await new Promise((resolve, reject) => {
      let output = "";
      server.stdout.on("data", chunk => {
        output += chunk;
        const match = output.match(/^(http:\/\/127\.0\.0\.1:\d+\/)$/m);
        if (match) resolve(match[1]);
      });
      server.once("error", reject);
      server.once("exit", code => reject(new Error(`Server exited: ${code}`)));
    });
    browser = await chromium.launch({ channel: "chrome", headless: true, chromiumSandbox: true });
    return await use(url, browser);
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null && server.signalCode === null) {
      const stopped = once(server, "exit");
      server.kill("SIGTERM");
      await stopped;
    }
  }
}

module.exports = { withPreview };
