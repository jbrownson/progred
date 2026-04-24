import { builtinModules } from "node:module"
import { defineConfig } from "vite"
import { progredDataPlugin } from "./progred"

export default defineConfig({
  plugins: [progredDataPlugin()],
  build: {
    outDir: "build",
    emptyOutDir: false,
    lib: {
      entry: "src/graph/generator.ts",
      formats: ["cjs"],
      fileName: () => "generator.cjs",
    },
    rollupOptions: {
      external: [...builtinModules, ...builtinModules.map(module => `node:${module}`)],
    },
  },
})
