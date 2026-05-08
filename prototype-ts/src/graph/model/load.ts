import { Maybe, nothing, unsafeUnwrapMaybe } from "../../lib/Maybe"
import { tuple } from "../../lib/Tuple"
import { GUIDMap } from "./GUIDMap"
import { GUID, ID, nidFromNumber, sidFromString } from "./ID"
import type { JSONID, SerializedGraph } from "./save"

// TODO error checking
export function load({root, guidMap}: SerializedGraph): {root: Maybe<ID>, guidMap: GUIDMap} {
  return {root, guidMap: new GUIDMap(new Map(Object.keys(guidMap).map(guid => tuple(guid, new Map(guidMap[guid].map(({label, to}) =>
    tuple(unsafeUnwrapMaybe(jsonToID(label)), unsafeUnwrapMaybe(jsonToID(to)))))))))} }

function jsonToID(json: JSONID): Maybe<ID> {
  return "guid" in json ? json.guid as GUID : "string" in json ? sidFromString(json.string) : "number" in json ? nidFromNumber(json.number) : nothing }
