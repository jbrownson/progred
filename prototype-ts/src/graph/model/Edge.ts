import type { ID } from "./ID"

export type Edge = {
  parent: ID
  label: ID
}

export function edgesEqual(a: Edge, b: Edge): boolean {
  return a.parent === b.parent && a.label === b.label
}
