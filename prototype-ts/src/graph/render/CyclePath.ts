import { ID } from "../model/ID"

export type CyclePath = ReadonlySet<ID>
export type CycleStep = {path: CyclePath, hasCycle: boolean}

export function emptyCyclePath(): CyclePath { return new Set() }

export function stepCyclePath(path: CyclePath, id: ID): CycleStep {
  return path.has(id)
    ? {path, hasCycle: true}
    : {path: new Set([...path, id]), hasCycle: false} }
