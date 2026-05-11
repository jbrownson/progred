import { readFile } from "node:fs/promises"
import type { Plugin } from "vite"

export function progredDataPlugin(): Plugin {
  return {
    name: "progred-data",
    async load(id) {
      const [file] = id.split("?")
      if (!file.endsWith(".progred")) return null
      return `export default ${await readFile(file, "utf8")}`
    },
  }
}
