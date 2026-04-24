import { Maybe, unsafeUnwrapMaybe } from "../../lib/Maybe"
import { GUIDMap } from "../GUIDMap"
import { ID } from "../ID"
import { IDMap } from "../IDMap"
import { load } from "../load"
import apps from "./Apps.progred"
import bradParams from "./BradParams.progred"
import evaluate from "./Evaluate.progred"
import javascript from "./Javascript.progred"
import json from "./JSON.progred"
import loadJson from "./LoadJSON.progred"
import render from "./Render.progred"
import typeLibrary from "./type.progred"

const loads: {[key: string]: {root: Maybe<ID>, guidMap: GUIDMap}} = {
  Type: load(typeLibrary),
  BradParams: load(bradParams),
  JSON: load(json),
  LoadJSON: load(loadJson),
  JavaScript: load(javascript),
  Render: load(render),
  Evaluate: load(evaluate),
  Apps: load(apps) }

export type Library = {idMap: IDMap, root: ID}
export const libraries = new Map<string, Library>(Object.keys(loads).map(key =>
  [key, {idMap: unsafeUnwrapMaybe(loads[key].guidMap), root: unsafeUnwrapMaybe(loads[key].root)}] as [string, Library] ))
