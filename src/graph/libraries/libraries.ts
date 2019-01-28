import { Maybe, unsafeUnwrapMaybe } from "../../lib/Maybe"
import { GUIDMap } from "../GUIDMap"
import { ID } from "../ID"
import { IDMap } from "../IDMap"
import { load } from "../load"

const loads: {[key: string]: {root: Maybe<ID>, guidMap: GUIDMap}} = {
  Type: load(require("./type.progred")),
  BradParams: load(require("./BradParams.progred")),
  JSON: load(require("./JSON.progred")),
  LoadJSON: load(require("./LoadJSON.progred")),
  JavaScript: load(require("./Javascript.progred")),
  Render: load(require("./Render.progred")),
  Evaluate: load(require("./Evaluate.progred")),
  AWS: load(require("./AWS.progred")),
  Apps: load(require("./Apps.progred")) }

export type Library = {idMap: IDMap, root: ID}
export const libraries = new Map<string, Library>(Object.keys(loads).map(key =>
  [key, {idMap: unsafeUnwrapMaybe(loads[key].guidMap), root: unsafeUnwrapMaybe(loads[key].root)}] as [string, Library] ))