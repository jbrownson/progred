// Cloudflare Web Analytics. Other hosts would only log a CORS error, since
// Cloudflare accepts reports from prog.red alone, and an embedded editor is
// part of a page that already counted the visit.
if (location.hostname === "prog.red" && window.top === window) {
  const beacon = document.createElement("script");
  beacon.type = "module";
  beacon.src = "https://static.cloudflareinsights.com/beacon.min.js";
  beacon.dataset.cfBeacon = JSON.stringify({ token: "30f3c6088e5243a1a25539de20290b11" });
  document.head.append(beacon);
}
