import { noopECallbacks } from "./editor/ECallbacks"
import type { EdgeContext } from "./editor/EditorCommands"
import { Environment, SourceID, withEnvironment } from "./Environment"
import { GUIDRootViews } from "./graph"
import type { ID } from "./model/ID"
import type { IDMap } from "./model/IDMap"
import { GUIDMap } from "./model/GUIDMap"
import { D, DText } from "./render/D"
import { Cursor } from "./cursor/Cursor"
import type { Maybe } from "../lib/Maybe"
import { SparseSpanningTree } from "./SparseSpanningTree"

type TestEnvironmentOptions = {
  libraries?: Map<string, {idMap: IDMap, root: ID}>
  guidMap?: GUIDMap
  rootViews?: GUIDRootViews
  sparseSpanningTree?: SparseSpanningTree
  defaultRender?: (cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext) => D
}

export function makeTestEnvironment(options: TestEnvironmentOptions = {}) {
  return new Environment(
    options.libraries || new Map(),
    options.guidMap || new GUIDMap(),
    options.rootViews || new GUIDRootViews("guid-root-views"),
    options.sparseSpanningTree || new SparseSpanningTree(),
    options.defaultRender || (() => new DText("")),
    noopECallbacks)
}

export function withTestEnvironment<A>(f: (environment: Environment) => A, options: TestEnvironmentOptions = {}) {
  const environment = makeTestEnvironment(options)
  return withEnvironment(environment, () => f(environment))
}
