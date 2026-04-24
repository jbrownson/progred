import { builtinModules } from "node:module"
import { defineConfig } from "vite"

export default defineConfig({
  build: {
    outDir: "build",
    emptyOutDir: false,
    lib: {
      entry: "src/electron/main.ts",
      formats: ["cjs"],
      fileName: () => "main.cjs",
    },
    rollupOptions: {
      external: ["electron", ...builtinModules, ...builtinModules.map(module => `node:${module}`)],
    },
  },
})
