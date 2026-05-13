import { noopECallbacks } from "./editor/ECallbacks"
import type { EdgeContext } from "./editor/EditorCommands"
import { Environment, SourceID, Workspace, withEnvironment } from "./Environment"
import type { GUID, ID } from "./model/ID"
import type { IDMap } from "./model/IDMap"
import { GUIDMap } from "./model/GUIDMap"
import type { D } from "./render/D"
import { dText } from "./render/DLayout"
import { Cursor } from "./cursor/Cursor"
import type { Maybe } from "../lib/Maybe"

type TestEnvironmentOptions = {
  libraries?: Map<string, {idMap: IDMap, root: ID}>
  guidMap?: GUIDMap
  workspace?: Workspace
  workspaceID?: GUID
  root?: Maybe<ID>
  view?: Maybe<ID>
  defaultRender?: (cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext) => D
}

export function makeTestEnvironment(options: TestEnvironmentOptions = {}) {
  let workspace = options.workspace || {id: options.workspaceID || "guid-workspace", root: options.root, view: options.view}
  return new Environment(
    options.libraries || new Map(),
    options.guidMap || new GUIDMap(),
    workspace,
    options.defaultRender || (() => dText("")),
    noopECallbacks)
}

export function withTestEnvironment<A>(f: (environment: Environment) => A, options: TestEnvironmentOptions = {}) {
  const environment = makeTestEnvironment(options)
  return withEnvironment(environment, () => f(environment))
}
