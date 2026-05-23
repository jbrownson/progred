import { Maybe, unsafeUnwrapMaybe } from "../../lib/Maybe"
import { GUIDMap } from "../model/GUIDMap"
import { ID } from "../model/ID"
import { IDMap } from "../model/IDMap"
import { load } from "../model/load"
import apps from "./Apps.progred"
import bradParams from "./BradParams.progred"
import evaluate from "./Evaluate.progred"
import javascript from "./Javascript.progred"
import json from "./JSON.progred"
import render from "./Render.progred"
import scene3D from "./Scene3D.progred"
import typeLibrary from "./type.progred"

const loads: {[key: string]: {root: Maybe<ID>, guidMap: GUIDMap}} = {
  Type: load(typeLibrary),
  BradParams: load(bradParams),
  JSON: load(json),
  JavaScript: load(javascript),
  Render: load(render),
  Scene3D: load(scene3D),
  Evaluate: load(evaluate),
  Apps: load(apps) }

export type Library = {idMap: IDMap, root: ID}
export const libraries = new Map<string, Library>(Object.keys(loads).map(key =>
  [key, {idMap: unsafeUnwrapMaybe(loads[key].guidMap), root: unsafeUnwrapMaybe(loads[key].root)}] as [string, Library] ))
