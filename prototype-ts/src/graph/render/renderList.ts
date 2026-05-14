import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { buildEntries } from "../editor/buildEntries"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { cursorHasCycle } from "../cursor/cursorHasCycle"
import { set } from "../Environment"
import { GUIDEmptyList, GUIDNonemptyList, HasID, headField, EmptyList, List, listFromID, ListType, matchList, NonemptyList, tailField } from "../graph"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { edgeContextForEdge, edgeContextFromEdge } from "../editor/edgeContext"
import { requestFocusForCursor, requestNextTabStopFromCursor } from "../editor/EditorFocus"
import { guidFromID, ID, matchID } from "../model/ID"
import { collapsible, collapseToggle } from "./DControls"
import { dList, type ListInsertionPoint } from "./DLayout"
import { alwaysFail, descend, Render } from "./R"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"

type ListProjectionItem<A extends HasID> = {cursor: Cursor, edgeContext: EdgeContext, list: NonemptyList<A>}
type ListProjection<A extends HasID> = {
  items: ListProjectionItem<A>[]
  emptyTailCursor: Cursor
  emptyTailEdgeContext: EdgeContext
  emptyTail: EmptyList
}

function listProjectionFromList<A extends HasID>(cursor: Cursor, edgeContext: EdgeContext, list: List<A>, visited = new Set<ID>()): Maybe<ListProjection<A>> {
  if (visited.has(list.id)) return nothing
  visited.add(list.id)
  return matchList(list,
    nonemptyList => bindMaybe(nonemptyList.tail, tail => {
      let tailCursor = _childCursor(cursor, nonemptyList.id, tailField.id)
      return mapMaybe(listProjectionFromList(tailCursor, edgeContextFromEdge({parent: nonemptyList.id, label: tailField.id}, edgeContext.expectedType), tail, visited), ({items, emptyTailCursor, emptyTailEdgeContext, emptyTail}) =>
        ({items: [{cursor, edgeContext, list: nonemptyList}, ...items], emptyTailCursor, emptyTailEdgeContext, emptyTail}) )}),
    emptyList => ({items: [], emptyTailCursor: cursor, emptyTailEdgeContext: edgeContext, emptyTail: emptyList}) )}

function listElementType(edgeContext: EdgeContext) {
  return bindMaybe(edgeContext.expectedType, type => type instanceof ListType ? type.type : nothing) }

export function listCreationEditorCommands(cursor: Cursor): EditorCommands {
  return {keyDown: e => e.key === "[" ? mapMaybe(e.commit, commit => () => {
    e.preventDefault()
    e.stopPropagation()
    let tail = GUIDEmptyList.new()
    let newList = GUIDNonemptyList.new(id => ({id})).setTail(tail)
    commit(newList.id)
    requestFocusForCursor(_childCursor(cursor, newList.id, headField.id)) }) : nothing} }

export function renderListParens(separator = ",", r = alwaysFail) { return renderList("(", ")", separator, r) }
export function renderListCurly(separator = ",", r = alwaysFail) { return renderList("{", "}", separator, r) }

export function renderList(opening = "[", closing = "]", separator = ",", r = alwaysFail): Render {
  return (listCursor, sourceID, listEdgeContext) => bindMaybe(sourceID, sourceID => {
    const edgeContext = fromMaybe(listEdgeContext, () => edgeContextForEdge(listCursor))
    return bindMaybe(listFromID(sourceID.id, id => ({id})), list =>
      mapMaybe(listProjectionFromList(listCursor, edgeContext, list), ({items, emptyTailCursor, emptyTailEdgeContext, emptyTail}) => {
        let defaultCollapsed = cursorHasCycle(listCursor)
        let render = (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => {
          let toggle = list instanceof NonemptyList ? collapseToggle(collapsed, () => setCollapsed(!collapsed)) : nothing
          if (collapsed && toggle) return renderDocumentGuidEditor(listCursor, sourceID, dList(opening, [], closing, separator, toggle, [], collapsed))
          let insertionPoint = (cursor: Cursor, edgeContext: EdgeContext, oldTail: List<HasID>, requiresMeta = false): ListInsertionPoint => {
            let insert = (id: Maybe<ID>) => mapMaybe(edgeContext.commit, commit => {
              let newList = GUIDNonemptyList.new(id => ({id})).setTail(oldTail)
              mapMaybe(id, id => newList.setHead({id}))
              commit(newList.id)
              requestNextTabStopFromCursor(_childCursor(cursor, newList.id, headField.id)) })
            return {
              entries: buildEntries(listElementType(edgeContext), id => insert(id())),
              editorCommands: {commit: insert},
              requiresMeta }}
          let listItem = (cursor: Cursor, listEdgeContext: EdgeContext, list: NonemptyList<HasID>) => {
            let commit = (id: Maybe<ID>) => maybe(id,
              () => mapMaybe(list.tail, tail => mapMaybe(listEdgeContext.commit, commit => commit(tail.id))),
              id => mapMaybe(guidFromID(list.id), guid => set(guid, headField.id, id)) )
            return descend(cursor, list.id, headField.id, r, {commit, expectedType: listElementType(listEdgeContext)}) }
          let requiresMetaAfter = (list: NonemptyList<HasID>) => maybe(list.head, () => false, head => matchID(head.id, () => false, () => true, () => false))
          let insertionPoints = [
            ...items.map(({cursor, edgeContext, list}, i) => insertionPoint(cursor, edgeContext, list, i !== 0 && requiresMetaAfter(items[i - 1].list))),
            insertionPoint(emptyTailCursor, emptyTailEdgeContext, emptyTail, maybe(items[items.length - 1], () => false, ({list}) => requiresMetaAfter(list))) ]
          return renderDocumentGuidEditor(listCursor, sourceID, dList(opening, items.map(({cursor, edgeContext, list}) => listItem(cursor, edgeContext, list)), closing, separator, toggle,
            insertionPoints, collapsed), listRootEditorCommands()) }
        return defaultCollapsed || list instanceof NonemptyList ? collapsible(defaultCollapsed, defaultCollapsed, render) : render(false, () => {}) }))})}

function listRootEditorCommands(): EditorCommands {
  return {keyDown: e => e.key === "," ? () => {
    e.preventDefault()
    e.stopPropagation()
    if (e.target instanceof HTMLElement) {
      const insertionPoint = e.target.querySelector("[data-list-insertion-index='0']")
      if (insertionPoint instanceof HTMLElement) insertionPoint.focus() }} : nothing}
}
