import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing, unsafeUnwrapMaybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { D } from "./D"
import { ECallbacks } from "./ECallbacks"
import { ctorField, GUIDRootViews, nameField } from "./graph"
import { GUIDMap } from "./GUIDMap"
import { GUID, guidFromID, ID, stringFromID } from "./ID"
import { IDMap } from "./IDMap"
import { _Selection } from "./Selection"
import { SparseSpanningTree } from "./SparseSpanningTree"

export class Environment {
  constructor(
    public libraries: Map<string, {idMap: IDMap, root: ID}>,
    public guidMap: GUIDMap,
    public rootViews: GUIDRootViews /* TODO this should exist outside the GUIDMap or something */,
    public sparseSpanningTree: SparseSpanningTree,
    private _selection: {selection: Maybe<_Selection>},
    public defaultRender: (cursor: Cursor, sourceID: Maybe<SourceID>) => D,
    public callbacks: ECallbacks ) {}
  get selection() { this.callbacks.onGetSelection(); return this._selection.selection }
  set selection(selection: Maybe<_Selection>) { this.callbacks.willSetSelection(selection); this._selection.selection = selection } }

let _environment: Maybe<Environment> = nothing
export function environment() { return unsafeUnwrapMaybe(_environment) }
export function withEnvironment<A>(newEnvironment: Environment, f: () => A) {
  let oldEnvironment = _environment
  _environment = newEnvironment
  let a = f()
  _environment = oldEnvironment
  return a }

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
  return bindMaybe(edges(id), ({edges, source}) => mapMaybe(edges.get(label), id => ({id, source}))) }

export function edges(id: ID): Maybe<{edges: Map<ID, ID>, source: Source}> {
  let e = environment()
  e.callbacks.onEdges(id)
  return altMaybe(
    bindMaybe(guidFromID(id), guid => mapMaybe(e.guidMap.edges(guid), edges => ({edges, source: {source: SourceType.DocumentType, guid} as Source}))),
    () => {
      for (let library of e.libraries.values()) {
        let edges = library.idMap.edges(id)
        if (edges !== nothing) return {edges, source: {source: SourceType.LibraryType} as Source} }})}

export function set(guid: GUID, label: ID, to: ID) {
  let e = environment()
  e.callbacks.willSet(guid, label, to)
  e.guidMap.set(guid, label, to) }
export function _delete(guid: GUID, label: ID) {
  let e = environment()
  e.callbacks.willDelete(guid, label)
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
export function logSelection() {
  let e = environment()
  console.log(maybe(e.selection, () => "No Selection", selection =>
    maybe(_get(selection.cursor.parent, selection.cursor.label), () => "Invalid Selection", getDebugID))) }