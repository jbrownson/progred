const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

function run(hostname, embedded) {
  const appended = [];
  const window = {};
  window.top = embedded ? {} : window;
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "public/analytics.js"), "utf8"), {
    location: { hostname },
    window,
    document: {
      createElement: (tag) => ({ tag, dataset: {} }),
      head: { append: (element) => appended.push(element) },
    },
  });
  return appended;
}

test("a page on prog.red loads Cloudflare's beacon with the site token", () => {
  const appended = run("prog.red", false);
  assert.equal(appended.length, 1);
  const [beacon] = appended;
  assert.equal(beacon.tag, "script");
  assert.equal(beacon.type, "module");
  assert.equal(beacon.src, "https://static.cloudflareinsights.com/beacon.min.js");
  assert.match(JSON.parse(beacon.dataset.cfBeacon).token, /^[0-9a-f]{32}$/);
});

test("embedded editors and local previews load nothing", () => {
  assert.deepEqual(run("prog.red", true), []);
  for (const hostname of ["localhost", "127.0.0.1"]) assert.deepEqual(run(hostname, false), []);
});
