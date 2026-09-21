// Each instance has its own JS globals; only Rust's WASM memory is shared.
let worker;
let notify;
let wakeScheduled = false;
let failed = false;
let channel;

function notifications(name) {
  // Rayon owns its nested workers. A per-instance channel lets any rendering
  // thread notify the UI without depending on that worker topology.
  return channel ??= new BroadcastChannel(name);
}

export function submitJob(pointer, name) {
  if (failed) throw new Error("Computation worker failed; reload the page");
  if (worker) worker.postMessage({ type: "job", pointer });
  else notifications(name).postMessage({ type: "submit", pointer });
}

export function workerWake(name) {
  if (notify) scheduleWake();
  else notifications(name).postMessage({ type: "wake" });
}

function scheduleWake() {
  if (failed || wakeScheduled) return;
  wakeScheduled = true;
  requestAnimationFrame(() => {
    wakeScheduled = false;
    if (!failed) notify();
  });
}

export async function startWorker(wasm, moduleUrl, onWake, onError,
  threads = Math.max(1, Math.min(8, (navigator.hardwareConcurrency ?? 2) - 1))) {
  if (!globalThis.crossOriginIsolated) {
    throw new Error("Background rendering needs cross-origin isolation (COOP/COEP headers from the website host).");
  }
  if (worker) throw new Error("Computation worker already started");
  if (!Number.isInteger(threads) || threads < 1) throw new Error("Rendering worker count must be a positive integer");
  const channelName = `progred-${crypto.randomUUID()}`;
  wasm.set_worker_channel(channelName);
  notifications(channelName).onmessage = ({ data }) => {
    if (failed) return;
    if (data.type === "wake") scheduleWake();
    else if (data.type === "submit") submitJob(data.pointer, channelName);
  };
  notify = onWake;
  worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  await new Promise((resolve, reject) => {
    let startupTimer;
    const fail = (error) => {
      if (failed) return;
      failed = true;
      clearTimeout(startupTimer);
      channel.close();
      worker.terminate();
      reject(error);
      onError(error);
    };
    worker.onerror = (event) => fail(new Error(event.message));
    worker.onmessageerror = () => fail(new Error("Invalid computation worker message"));
    worker.onmessage = ({ data }) => {
      if (data.type === "ready") { clearTimeout(startupTimer); resolve(); }
      else if (data.type === "failed") fail(new Error(data.message));
    };
    startupTimer = setTimeout(() => fail(new Error("Rendering workers did not start within 60 seconds")), 60000);
    worker.postMessage({
      type: "init", moduleUrl, threads,
      module: wasm.worker_module(), memory: wasm.worker_memory(),
    });
  });
}
