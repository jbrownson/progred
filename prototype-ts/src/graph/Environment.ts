import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing, unsafeUnwrapMaybe } from "../lib/Maybe"
import { Cursor } from "./cursor/Cursor"
import type { D } from "./render/ProjectionContext"
import { ECallbacks } from "./editor/ECallbacks"
import { ctorField, nameField } from "./graph"
import { GUIDMap } from "./model/GUIDMap"
import { GUID, guidFromID, ID, stringFromID } from "./model/ID"
import { IDMap } from "./model/IDMap"
import type { EdgeContext } from "./editor/EditorCommands"
import { workspaceRootField, workspaceViewField } from "./workspace"

export type Workspace = {
  id: GUID
  root: Maybe<ID>
  view: Maybe<ID>
}

export class Environment {
  constructor(
    public libraries: Map<string, {idMap: IDMap, root: ID}>,
    public guidMap: GUIDMap,
    public workspace: Workspace,
    public defaultRender: (cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext) => D,
    public callbacks: ECallbacks ) {} }

let _environment: Maybe<Environment> = nothing
export function environment() { return unsafeUnwrapMaybe(_environment) }
export function withEnvironment<A>(newEnvironment: Environment, f: () => A) {
  let oldEnvironment = _environment
  _environment = newEnvironment
  try {
    return f()
  } finally {
    _environment = oldEnvironment }}

export const enum SourceType {
  DocumentType,
  LibraryType }

export type DocumentSource = { source: SourceType.DocumentType, guid: GUID }
export type LibrarySource = { source: SourceType.LibraryType }
export type Source = DocumentSource | LibrarySource
export type SourceID = {id: ID, source: Source}
export function documentSourceFromSource(source: Source): Maybe<DocumentSource> {
  return source.source === SourceType.DocumentType
    ? source
    : nothing }
export function librarySourceFromSource(source: Source): Maybe<LibrarySource> {
  return source.source === SourceType.LibraryType
    ? source
    : nothing }
export function guidFromSource(source: Source) { return mapMaybe(documentSourceFromSource(source), s => s.guid) }

export function _get(id: ID, label: ID): Maybe<ID> { return mapMaybe(get(id, label), ({id}) => id) }
export function get(id: ID, label: ID): Maybe<SourceID> {
  let e = environment()
  e.callbacks.onGet(id, label)
  if (id === e.workspace.id)
    return mapMaybe(workspaceGet(label), id => ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: e.workspace.id}}))
  return bindMaybe(edges(id), ({edges, source}) => mapMaybe(edges.get(label), id => ({id, source}))) }

export function edges(id: ID): Maybe<{edges: Map<ID, ID>, source: Source}> {
  let e = environment()
  e.callbacks.onEdges(id)
  if (id === e.workspace.id) {
    let edges = new Map<ID, ID>()
    mapMaybe(e.workspace.root, root => edges.set(workspaceRootField.id, root))
    mapMaybe(e.workspace.view, view => edges.set(workspaceViewField.id, view))
    return edges.size > 0 ? {edges, source: {source: SourceType.DocumentType, guid: e.workspace.id}} : nothing }
  return altMaybe(
    bindMaybe(guidFromID(id), guid => mapMaybe(e.guidMap.edges(guid), edges => ({edges, source: {source: SourceType.DocumentType, guid} as Source}))),
    () => {
      for (let library of e.libraries.values()) {
        let edges = library.idMap.edges(id)
        if (edges !== nothing) return {edges, source: {source: SourceType.LibraryType} as Source} }})}

function workspaceGet(label: ID): Maybe<ID> {
  let e = environment()
  return label === workspaceRootField.id ? e.workspace.root : label === workspaceViewField.id ? e.workspace.view : nothing }

function workspaceSet(label: ID, to: ID): boolean {
  let e = environment()
  if (label === workspaceRootField.id) {
    e.workspace.root = to
    return true }
  if (label === workspaceViewField.id) {
    e.workspace.view = to
    return true }
  return false }

function workspaceDelete(label: ID): boolean {
  let e = environment()
  if (label === workspaceRootField.id) {
    e.workspace.root = nothing
    return true }
  if (label === workspaceViewField.id) {
    e.workspace.view = nothing
    return true }
  return false }

export function set(guid: GUID, label: ID, to: ID) {
  let e = environment()
  e.callbacks.willSet(guid, label, to)
  if (guid === e.workspace.id && workspaceSet(label, to)) return
  e.guidMap.set(guid, label, to) }
export function _delete(guid: GUID, label: ID) {
  let e = environment()
  e.callbacks.willDelete(guid, label)
  if (guid === e.workspace.id && workspaceDelete(label)) return
  e.guidMap.delete(guid, label) }
export function setOrDelete(guid: GUID, label: ID, to: Maybe<ID>) { return maybe(to, () => _delete(guid, label), to => set(guid, label, to)) }

export function getDebugIDName(id: ID, visited = new Set<ID>()): string {
  if (visited.has(id)) { return "" }
  visited.add(id)
  return fromMaybe(stringFromID(fromMaybe(_get(id, nameField.id), () => id)),
    () => id + maybe(_get(id, ctorField.id), () => "", id => ": " + getDebugIDName(id, visited)) )}

export function getDebugID(id: ID): string {
  return [getDebugIDName(id), ...maybe(edges(id), () => [], ({edges}) => Array.from(edges).map(([k, v]) => `\t"label": "${getDebugIDName(k)}", "value": "${getDebugIDName(v)}"`))].join('\n') }

export function logID(id: ID) { console.log(getDebugID(id)) }
