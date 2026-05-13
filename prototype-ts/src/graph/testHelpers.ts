import { noopECallbacks } from "./editor/ECallbacks"
import type { EdgeContext } from "./editor/EditorCommands"
import { Environment, SourceID, withEnvironment } from "./Environment"
import { GUIDRootViews } from "./graph"
import type { ID } from "./model/ID"
import type { IDMap } from "./model/IDMap"
import { GUIDMap } from "./model/GUIDMap"
import { D, dText } from "./render/Projection"
import { Cursor } from "./cursor/Cursor"
import type { Maybe } from "../lib/Maybe"

type TestEnvironmentOptions = {
  libraries?: Map<string, {idMap: IDMap, root: ID}>
  guidMap?: GUIDMap
  rootViews?: GUIDRootViews
  defaultRender?: (cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext) => D
}

export function makeTestEnvironment(options: TestEnvironmentOptions = {}) {
  return new Environment(
    options.libraries || new Map(),
    options.guidMap || new GUIDMap(),
    options.rootViews || new GUIDRootViews("guid-root-views"),
    options.defaultRender || (() => dText("")),
    noopECallbacks)
}

export function withTestEnvironment<A>(f: (environment: Environment) => A, options: TestEnvironmentOptions = {}) {
  const environment = makeTestEnvironment(options)
  return withEnvironment(environment, () => f(environment))
}
