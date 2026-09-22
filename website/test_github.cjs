const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

async function load(response) {
  const count = { hidden: true, textContent: "" };
  const link = { setAttribute(name, value) { this[name] = value; } };
  let requests = 0;
  await vm.runInNewContext(fs.readFileSync(path.join(__dirname, "public/github.js"), "utf8"), {
    AbortSignal,
    fetch: async (url, options) => {
      requests++;
      assert.equal(url, "https://api.github.com/repos/jbrownson/progred");
      assert.equal(options.credentials, "omit");
      if (response instanceof Error) throw response;
      return response;
    },
    document: { querySelector: (selector) => selector === "#github-count" ? count : link },
  });
  assert.equal(requests, 1);
  return { count, link };
}

test("shows a valid count, including zero, with an accessible description", async () => {
  for (const stars of [0, 42]) {
    const { count, link } = await load({ ok: true, json: async () => ({ stargazers_count: stars }) });
    assert.equal(count.hidden, false);
    assert.equal(count.textContent, String(stars));
    assert.equal(link["aria-label"], `View progred on GitHub (${stars} stars)`);
    assert.equal(link.title, link["aria-label"]);
  }
});

test("network failures, rate limits, and invalid counts leave an icon-only link", async () => {
  for (const response of [
    new Error("Offline"), { ok: false },
    { ok: true, json: async () => { throw new Error("Invalid JSON"); } },
    ...[undefined, -1, "42", 1.5].map((stars) => ({ ok: true, json: async () => ({ stargazers_count: stars }) })),
  ]) {
    const { count, link } = await load(response);
    assert.equal(count.hidden, true);
    assert.equal(count.textContent, "");
    assert.equal(link["aria-label"], undefined);
  }
});
