import type { ProgredApi } from "./preload"

declare global {
  interface Window {
    progred: ProgredApi
  }
}

export {}
