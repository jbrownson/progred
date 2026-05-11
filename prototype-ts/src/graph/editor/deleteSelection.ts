import { altMaybe, bindMaybe, booleanFromMaybe, firstMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { cursorFromD } from "../cursor/cursorFromD"
import { descendFromCursor } from "../cursor/descendFromCursor"
import { _delete, documentSourceFromSource, environment, get } from "../Environment"
import { guidFromID } from "../model/ID"
import { createD, D, Descend } from "../render/D"
import type { DeleteDirection } from "./EditorCommands"
import { deleteListElemCursor, selectionCursorBindMaybe } from "./listCursorActions"

export function deleteCursorDefault(cursor: Cursor): boolean {
  return booleanFromMaybe(bindMaybe(guidFromID(cursor.parent), guid =>
      bindMaybe(get(guid, cursor.label), ({id, source}) =>
      mapMaybe(documentSourceFromSource(source), source => {
          _delete(guid, cursor.label)
          return {} }))))}

function composeDeletes(...deleters: ((cursor: Cursor) => boolean)[]) {
  return (cursor: Cursor): boolean => {
    return deleters.length === 0
      ? false
      : deleters[0](cursor) || composeDeletes(...deleters.slice(1))(cursor) } }

const deleteHandler = composeDeletes(deleteListElemCursor, deleteCursorDefault)

export function deleteCursor(cursor: Cursor): boolean { return deleteHandler(cursor) }

export function deleteSelection(): boolean { return fromMaybe(selectionCursorBindMaybe(deleteCursor), () => false) }

function siblingIndex(d: D): Maybe<number> { return bindMaybe(d.parent, parent => parent.children.findIndex(child => child === d)) }

function parentDescend(d: D): Maybe<Descend> {
  return bindMaybe(d.parent, parent => parent instanceof Descend ? parent : parentDescend(parent)) }

function goDown(d: D): Maybe<Descend> {
  return firstMaybe(d.children.map(child => (): Maybe<Descend> => altMaybe(child instanceof Descend ? child : nothing, () => goDown(child)))) }

function getSibling(d: D, n: number): Maybe<D> {
  return bindMaybe(d.parent, parent => bindMaybe(parent.children.findIndex(child => child === d), index => parent.children[index + n])) }

function parentCursorAndDSiblingIndex(cursor: Cursor, rootDescend: Descend, viewsDescend: Maybe<Descend>) : Maybe<{parentCursor: Cursor, dSiblingIndex: number}> {
  return bindMaybe(descendFromCursor(rootDescend, viewsDescend, cursor), descend =>
    bindMaybe(siblingIndex(descend), dSiblingIndex =>
      bindMaybe(bindMaybe(parentDescend(descend), pDescend => cursorFromD(pDescend)), parentCursor => ({parentCursor, dSiblingIndex})))) }

function parentCursorAndDSiblingIndexToCursor(pcds: {parentCursor: Cursor, dSiblingIndex: number}, rootDescend: Descend, viewsDescend: Maybe<Descend>, dSiblingOffset: number): Maybe<Cursor> {
  const {parentCursor, dSiblingIndex} = pcds
  const pDescend = descendFromCursor(rootDescend, viewsDescend, parentCursor)
  const siblingDescend = bindMaybe(pDescend, goDown)
  const newDescend = fromMaybe(
    bindMaybe(dSiblingIndex, sIndex =>
      bindMaybe(siblingDescend, d =>
        altMaybe(getSibling(d, sIndex + dSiblingOffset), () => getSibling(d, sIndex)))),
    () => fromMaybe(pDescend, () => rootDescend))
  return cursorFromD(newDescend) }

export function deleteCursorAndSelect(cursor: Cursor, rootDescend: Descend, viewsDescend: Maybe<Descend>, direction: DeleteDirection): boolean {
  const parentCursorAndSIndex = parentCursorAndDSiblingIndex(cursor, rootDescend, viewsDescend)
  if (deleteCursor(cursor)) {
    const {rootDescend, viewsDescend} = createD()
    const target = direction === "forward"
      ? mapMaybe(parentCursorAndSIndex, ({parentCursor, dSiblingIndex}) => ({parentCursor, dSiblingIndex: dSiblingIndex - 1}))
      : parentCursorAndSIndex
    environment().selection = altMaybe(bindMaybe(target, target =>
        mapMaybe(parentCursorAndDSiblingIndexToCursor(target, rootDescend, viewsDescend, direction === "forward" ? 1 : -1), cursor => ({cursor}))),
      () => mapMaybe(cursorFromD(rootDescend), cursor => ({cursor})))
    return true }
  return false }
