import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { buildEntries } from "../editor/buildEntries"
import { edges, set, Source, SourceType } from "../Environment"
import { GUIDEmptyList, GUIDNonemptyList, HasID, headField, EmptyList, List, listFromID, ListType, matchList, NonemptyList, tailField } from "../graph"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { edgeContextForEdge, edgeContextFromEdge } from "../editor/edgeContext"
import { parentEditorDescendElement, requestFocusParentFromActiveElement, requestNextTabStopFromActiveElement } from "../editor/EditorFocus"
import { Edge } from "../model/Edge"
import { guidFromID, ID, matchID } from "../model/ID"
import { collapsible, collapseToggle } from "./DControls"
import { dList, type ListInsertionPoint } from "./DLayout"
import { alwaysFail, descend, Render } from "./R"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { emptyCyclePath, stepCyclePath, type CyclePath } from "./CyclePath"

type ListProjectionItem<A extends HasID> = {edge: Edge, edgeContext: EdgeContext, cyclePath: CyclePath, list: NonemptyList<A>}
type ListProjection<A extends HasID> = {
  items: ListProjectionItem<A>[]
  emptyTailEdge: Edge
  emptyTailEdgeContext: EdgeContext
  emptyTail: EmptyList
}

function listProjectionFromList<A extends HasID>(edge: Edge, edgeContext: EdgeContext, cyclePath: CyclePath, list: List<A>, visited = new Set<ID>()): Maybe<ListProjection<A>> {
  if (visited.has(list.id)) return nothing
  visited.add(list.id)
  let tailCyclePath = stepCyclePath(cyclePath, list.id).path
  return matchList(list,
    nonemptyList => bindMaybe(nonemptyList.tail, tail => {
      let tailEdge = {parent: nonemptyList.id, label: tailField.id}
      return mapMaybe(listProjectionFromList(tailEdge, edgeContextFromEdge(tailEdge, edgeContext.expectedType), tailCyclePath, tail, visited), ({items, emptyTailEdge, emptyTailEdgeContext, emptyTail}) =>
        ({items: [{edge, edgeContext, cyclePath, list: nonemptyList}, ...items], emptyTailEdge, emptyTailEdgeContext, emptyTail}) )}),
    emptyList => ({items: [], emptyTailEdge: edge, emptyTailEdgeContext: edgeContext, emptyTail: emptyList}) )}

function listElementType(edgeContext: EdgeContext) {
  return bindMaybe(edgeContext.expectedType, type => type instanceof ListType ? type.type : nothing) }

export function listCreationEditorCommands(): EditorCommands {
  return {keyDown: e => e.key === "[" ? mapMaybe(e.commit, commit => () => {
    e.preventDefault()
    e.stopPropagation()
    let tail = GUIDEmptyList.new()
    let newList = GUIDNonemptyList.new(id => ({id})).setTail(tail)
    requestNextTabStopFromActiveElement()
    commit(newList.id) }) : nothing} }

export function renderListParens(separator = ",", r = alwaysFail) { return renderList("(", ")", separator, r) }
export function renderListCurly(separator = ",", r = alwaysFail) { return renderList("{", "}", separator, r) }

export function renderList(opening = "[", closing = "]", separator = ",", r = alwaysFail): Render {
  return (listEdge, sourceID, listEdgeContext, cyclePath = emptyCyclePath()) => bindMaybe(sourceID, sourceID => {
    const edgeContext = fromMaybe(listEdgeContext, () => edgeContextForEdge(listEdge))
    return bindMaybe(listFromID(sourceID.id, id => ({id})), list =>
      mapMaybe(listProjectionFromList(listEdge, edgeContext, cyclePath, list), ({items, emptyTailEdge, emptyTailEdgeContext, emptyTail}) => {
        const listEdges = edges(sourceID.id)
        const writable = maybe(listEdges, () => sourceIsWritable(sourceID.source), ({source}) => sourceIsWritable(source))
        let cycleStep = stepCyclePath(cyclePath, sourceID.id)
        let defaultCollapsed = cycleStep.hasCycle
        let render = (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => {
          let toggle = list instanceof NonemptyList ? collapseToggle(collapsed, () => setCollapsed(!collapsed)) : nothing
          if (collapsed && toggle) return renderDocumentGuidEditor(listEdge, sourceID, dList(opening, [], closing, separator, toggle, [], collapsed))
          let insertionPoint = (edge: Edge, edgeContext: EdgeContext, oldTail: List<HasID>, requiresMeta = false): ListInsertionPoint => {
            let insert = (id: Maybe<ID>) => mapMaybe(edgeContext.commit, commit => {
              let newList = GUIDNonemptyList.new(id => ({id})).setTail(oldTail)
              mapMaybe(id, id => newList.setHead({id}))
              commit(newList.id) })
            return {
              entries: () => buildEntries(listElementType(edgeContext), id => insert(id())),
              editorCommands: {commit: insert},
              requiresMeta }}
          let listItem = (listEdgeContext: EdgeContext, cyclePath: CyclePath, list: NonemptyList<HasID>) => {
            let commit = writable ? (id: Maybe<ID>) => maybe(id,
              () => mapMaybe(list.tail, tail => mapMaybe(listEdgeContext.commit, commit => {
                requestFocusParentFromActiveElement()
                commit(tail.id) })),
              id => mapMaybe(guidFromID(list.id), guid => set(guid, headField.id, id)) ) : undefined
            return descend(list.id, headField.id, r, {commit, expectedType: listElementType(listEdgeContext)}, cyclePath) }
          let requiresMetaAfter = (list: NonemptyList<HasID>) => maybe(list.head, () => false, head => matchID(head.id, () => false, () => true, () => false))
          let insertionPoints = writable ? [
            ...items.map(({edge, edgeContext, list}, i) => insertionPoint(edge, edgeContext, list, i !== 0 && requiresMetaAfter(items[i - 1].list))),
            insertionPoint(emptyTailEdge, emptyTailEdgeContext, emptyTail, maybe(items[items.length - 1], () => false, ({list}) => requiresMetaAfter(list))) ] : []
          return renderDocumentGuidEditor(listEdge, sourceID, dList(opening, items.map(({edgeContext, cyclePath, list}) => listItem(edgeContext, cyclePath, list)), closing, separator, toggle,
            insertionPoints, collapsed), writable ? listRootEditorCommands() : {}) }
        return defaultCollapsed || list instanceof NonemptyList ? collapsible(defaultCollapsed, defaultCollapsed, render) : render(false, () => {}) }))})}

function sourceIsWritable(source: Source) { return source.source === SourceType.DocumentType }

function listRootEditorCommands(): EditorCommands {
  return {keyDown: e => e.key === "," ? () => {
    e.preventDefault()
    e.stopPropagation()
    if (e.target instanceof HTMLElement) {
      let descendElement = parentEditorDescendElement(e.target)
      const insertionPoint = Array.from(e.target.querySelectorAll(".listInsertionPoint")).find((insertionPoint): insertionPoint is HTMLElement =>
        insertionPoint instanceof HTMLElement && parentEditorDescendElement(insertionPoint) === descendElement)
      if (insertionPoint instanceof HTMLElement) insertionPoint.focus() }} : nothing}
}
