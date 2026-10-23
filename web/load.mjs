// Workers share the editor's memory only on a cross-origin isolated page.
// Elsewhere the fallback build runs without them, so it has no background
// rendering for 3D toolpath previews.
export async function loadEditor(isolated) {
  const url = new URL(isolated ? "./pkg/progred.js" : "./pkg-fallback/progred.js", import.meta.url).href;
  const wasm = await import(url);
  await wasm.default();
  return { wasm, url };
}
