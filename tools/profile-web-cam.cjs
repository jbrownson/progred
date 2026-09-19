// Runs only the headless diagnostic page, never the editor.
const { spawn } = require("node:child_process");
const { once } = require("node:events");
const { chromium } = require("playwright");

(async () => {
  const size = Number(process.argv[2] ?? 512);
  const trials = Number(process.argv[3] ?? 4);
  const positions = (process.argv[4] ?? "0.02,0.5,1").split(",").map(Number);
  const controls = process.argv[5] === "controls";
  const threads = Number(process.argv[6] ?? 8);
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
      server.once("exit", (code) => reject(new Error(`Server exited: ${code}`)));
    });
    browser = await chromium.launch({ channel: "chrome", headless: true, chromiumSandbox: true });
    const page = await browser.newPage();
    page.on("pageerror", (error) => console.error(error));
    await page.goto(`${url}editor/cam-profile.html?threads=${threads}`);
    await page.waitForFunction(() => window.runProfile || window.profileError, null, { timeout: 60000 });
    const error = await page.evaluate(() => window.profileError);
    if (error) throw new Error(error);
    console.log(JSON.stringify({ browser: browser.version(), size, trials, positions, controls }));
    for (const progress of positions) {
      for (let trial = 0; trial < trials; trial++) {
        const result = await page.evaluate(async ({ progress, size, controls }) =>
          window.runProfile(progress, size, controls), { progress, size, controls });
        console.log(JSON.stringify({ trial, ...result }));
      }
    }
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null && server.signalCode === null) {
      const stopped = once(server, "exit");
      server.kill("SIGTERM");
      await stopped;
    }
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
