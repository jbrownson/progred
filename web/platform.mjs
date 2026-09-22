// Keyboard conventions follow the browser's host, not the Wasm build target.
export function commandIsMeta(platform) {
  return /^(Mac|iPhone|iPad|iPod)/.test(platform);
}
