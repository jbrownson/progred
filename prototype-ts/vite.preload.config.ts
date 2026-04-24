import { builtinModules } from "node:module"
import { defineConfig } from "vite"

export default defineConfig({
  build: {
    outDir: "build",
    emptyOutDir: false,
    lib: {
      entry: "src/electron/preload.ts",
      formats: ["cjs"],
      fileName: () => "preload.cjs",
    },
    rollupOptions: {
      external: ["electron", ...builtinModules, ...builtinModules.map(module => `node:${module}`)],
    },
  },
})
