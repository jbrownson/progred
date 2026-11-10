// The preview server and headless Chrome that the browser diagnostics run
// against. website/preview.py builds editor changes before serving.
const { once } = require("node:events");
// tools/package.json pins playwright-core; it drives the installed Chrome.
const { chromium } = require("playwright-core");
const { preview } = require("./web-control");

// Calls `use(url, browser)` with a fresh server and browser, and closes both
// afterwards. `serverErrors` is the server's stderr handling.
async function withPreview(use, { serverErrors = "inherit" } = {}) {
  const { server, url } = await preview(true, serverErrors);
  let browser;
  try {
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
