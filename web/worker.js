let wasm;

self.onmessage = async ({ data }) => {
  try {
    if (data.type === "init") {
      wasm = await import(data.moduleUrl);
      await wasm.default({ module_or_path: data.module, memory: data.memory });
      // Rayon may block waiting for its workers: initialize and use it on the
      // computation coordinator, never on the browser's UI thread.
      await wasm.initThreadPool(data.threads);
      self.postMessage({ type: "ready" });
    } else if (data.type === "job") {
      wasm.run_worker_job(data.pointer);
    }
  } catch (error) {
    // A WASM trap is not a recoverable Rust job error. Do not keep processing
    // pointers after a trap; the page shows the error and requires a reload.
    self.postMessage({ type: "failed", message: error.stack ?? String(error) });
    self.close();
  }
};
