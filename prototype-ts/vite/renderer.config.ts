import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"
import { progredDataPlugin } from "./progred"

export default defineConfig({
  base: "./",
  plugins: [progredDataPlugin(), react()],
  build: {
    chunkSizeWarningLimit: 1500,
    outDir: "build/renderer",
    emptyOutDir: true,
    rollupOptions: {
      input: "grapheditor.html",
    },
  },
})
