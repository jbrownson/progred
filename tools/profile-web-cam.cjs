// Runs only the headless diagnostic page, never the editor.
const { withPreview } = require("./web-diagnostics.cjs");

(async () => {
  const [size, height = size] = (process.argv[2] ?? "512").split("x").map(Number);
  const trials = Number(process.argv[3] ?? 4);
  const positions = (process.argv[4] ?? "0.02,0.5,1").split(",").map(Number);
  const controls = process.argv[5] === "controls";
  const threads = Number(process.argv[6] ?? 8);
  const pkg = process.argv[7] ?? "profile-pkg";
  await withPreview(async (url, browser) => {
    const page = await browser.newPage();
    page.on("pageerror", (error) => console.error(error));
    await page.goto(`${url}editor/cam-profile.html?${new URLSearchParams({ threads, pkg })}`);
    await page.waitForFunction(() => window.runProfile || window.profileError, null, { timeout: 60000 });
    const error = await page.evaluate(() => window.profileError);
    if (error) throw new Error(error);
    console.log(JSON.stringify({ browser: browser.version(), size, height, trials, positions, controls, threads, pkg }));
    for (const progress of positions) {
      for (let trial = 0; trial < trials; trial++) {
        const result = await page.evaluate(async ({ progress, size, height, controls }) =>
          window.runProfile(progress, size, controls, height), { progress, size, height, controls });
        console.log(JSON.stringify({ trial, ...result }));
      }
    }
  });
})().catch((error) => { console.error(error); process.exitCode = 1; });
