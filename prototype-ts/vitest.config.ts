import { defineConfig } from "vitest/config"
import { progredDataPlugin } from "./vite/progred"

export default defineConfig({
  plugins: [progredDataPlugin()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"] }})
