import { Maybe, nothing, unsafeUnwrapMaybe } from "../../lib/Maybe"
import { GUIDMap } from "./GUIDMap"

let _guidMap: Maybe<GUIDMap> = nothing
export function guidMap(): GUIDMap { return unsafeUnwrapMaybe(_guidMap) }
export function withGUIDMap<A>(guidMap: GUIDMap, f: () => A) {
  let oldGUIDMap = _guidMap
  _guidMap = guidMap
  let a = f()
  _guidMap = oldGUIDMap
  return a }