import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"
import { progredDataPlugin } from "./vite.progred"

export default defineConfig({
  base: "./",
  plugins: [progredDataPlugin(), react()],
  build: {
    outDir: "build/renderer",
    emptyOutDir: true,
    rollupOptions: {
      input: "grapheditor.html",
    },
  },
})
