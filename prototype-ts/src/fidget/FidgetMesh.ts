import init, { mesh_implicit_json } from "./generated/progred_fidget"

export type FidgetMesh = {
  positions: number[]
  indices: number[]
}

export type FidgetImplicit =
  | {op: "x"}
  | {op: "y"}
  | {op: "z"}
  | {op: "constant", value: number}
  | {op: "add", a: FidgetImplicit, b: FidgetImplicit}
  | {op: "subtract", a: FidgetImplicit, b: FidgetImplicit}
  | {op: "multiply", a: FidgetImplicit, b: FidgetImplicit}
  | {op: "divide", a: FidgetImplicit, b: FidgetImplicit}
  | {op: "minimum", a: FidgetImplicit, b: FidgetImplicit}
  | {op: "maximum", a: FidgetImplicit, b: FidgetImplicit}

let implicitMeshCache = new Map<string, FidgetMesh>()
let initialized: Promise<unknown> | undefined

async function initialize() {
  initialized ||= init({module_or_path: wasmInput()})
  return initialized }

async function wasmInput() {
  const url = new URL("./generated/progred_fidget_bg.wasm", import.meta.url)
  if (url.protocol === "file:")
    return window.progred.readFileBytes(decodeURIComponent(url.pathname))
  return url }

export async function fidgetMeshFromImplicit(implicit: FidgetImplicit, depth = 5, scale = 2): Promise<FidgetMesh> {
  await initialize()
  const json = JSON.stringify({depth, scale, implicit})
  const cached = implicitMeshCache.get(json)
  if (cached !== undefined) return cached
  const mesh = JSON.parse(mesh_implicit_json(JSON.stringify(implicit), depth, scale))
  implicitMeshCache.set(json, mesh)
  return mesh
}
