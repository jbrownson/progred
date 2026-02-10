import { altMaybe, bindMaybe, booleanFromMaybe, firstMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { cursorFromD } from "./cursorFromD"
import { createD, D, Descend } from "./D"
import { deleteCursor } from "./deleteSelection"
import { descendFromCursor } from "./descendFromCursor"
import { _get, environment } from "./Environment"
import { findNextTabStop, findTabStop } from "./findNextTabStop"
import { matchID } from "./ID"
import { appendToListCursor, insertAfterListElemCursor, selectionCursorBindMaybe } from "./listCursorActions"

export type KeyHandler = (e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, rootDescend, viewsDescend, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, rootDescend, viewsDescend, runE))) }

function siblingIndex(d: D): Maybe<number> { return bindMaybe(d.parent, parent => parent.children.findIndex(child => child === d)) }
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

export function deleteKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Delete":
      return runE(() => booleanFromMaybe(selectionCursorBindMaybe(cursor => {
        e.stopPropagation()
        e.preventDefault()
        const parentCursorAndSIndexDesired = parentCursorAndDSiblingIndex(cursor, rootDescend, viewsDescend)
        if (deleteCursor(cursor)) {
          const {rootDescend, viewsDescend} = createD()
          const parentCursorAndSIndex = mapMaybe(parentCursorAndSIndexDesired, ({parentCursor, dSiblingIndex}) => ({parentCursor, dSiblingIndex: dSiblingIndex - 1}))
          environment().selection = altMaybe(bindMaybe(parentCursorAndSIndex, parentCursorAndSIndex =>
              mapMaybe(parentCursorAndDSiblingIndexToCursor(parentCursorAndSIndex, rootDescend, viewsDescend, 1), cursor => ({cursor}))),
            () => mapMaybe(cursorFromD(rootDescend), cursor => ({cursor})))
          return true }
        return false })))
    case "Backspace":
      return runE(() => booleanFromMaybe(selectionCursorBindMaybe(cursor => {
        e.stopPropagation()
        e.preventDefault()
        const parentCursorAndSIndex = parentCursorAndDSiblingIndex(cursor, rootDescend, viewsDescend)
        if (deleteCursor(cursor)) {
          let {rootDescend, viewsDescend} = createD()
          environment().selection = altMaybe(bindMaybe(parentCursorAndSIndex, parentCursorAndSIndex =>
              mapMaybe(parentCursorAndDSiblingIndexToCursor(parentCursorAndSIndex, rootDescend, viewsDescend, -1), cursor => ({cursor}))),
            () => mapMaybe(cursorFromD(rootDescend), cursor => ({cursor})))
          return true }
        return false })))}
  return false }

function parentDescend(d: D): Maybe<Descend> {
  return bindMaybe(d.parent, parent => parent instanceof Descend ? parent : parentDescend(parent)) }

function goDown(d: D): Maybe<Descend> {
  return firstMaybe(d.children.map(child => (): Maybe<Descend> => altMaybe(child instanceof Descend ? child : nothing, () => goDown(child)))) }

function getSibling(d: D, n: number): Maybe<D> {
  return bindMaybe(d.parent, parent => bindMaybe(parent.children.findIndex(child => child === d), index => parent.children[index + n])) }

function goLeftRight(d: D, n: number): Maybe<Descend> {
  return altMaybe(
    bindMaybe(getSibling(d, n), rightSibling =>
      rightSibling instanceof Descend ? rightSibling : altMaybe(goDown(rightSibling), () => goLeftRight(rightSibling, n)) ),
    () => bindMaybe(d.parent, parent => goLeftRight(parent, n)) )}

function selectDescend(descend: Descend) { environment().selection = {cursor: descend.cursor} }

// TODO makeElementVisible
function keyboardNav(f: (descend: Descend) => Maybe<Descend>, rootDescend: Descend, viewsDescend: Maybe<Descend>): boolean {
  let env = environment()
  return booleanFromMaybe(mapMaybe(bindMaybe(bindMaybe(env.selection, selection => descendFromCursor(rootDescend, viewsDescend, selection.cursor)), f), d => { selectDescend(d); return {} })) }

export function arrowNavKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault()
      return runE(() => keyboardNav(d => goLeftRight(d, -1), rootDescend, viewsDescend))
    case "ArrowRight":
      e.preventDefault()
      return runE(() => keyboardNav(d => goLeftRight(d, 1), rootDescend, viewsDescend))
    case "ArrowDown":
      e.preventDefault()
      return runE(() => maybe(environment().selection,
        () => { selectDescend(rootDescend); return true },
        () => booleanFromMaybe(keyboardNav(goDown, rootDescend, viewsDescend)) ))
    case "ArrowUp":
      e.preventDefault()
      return runE(() => booleanFromMaybe(bindMaybe(environment().selection, selection =>
        mapMaybe(bindMaybe(descendFromCursor(rootDescend, viewsDescend, selection.cursor), parentDescend), selectDescend) )))}
  return false }

export function doTab(shift: boolean, rootDescend: Descend, viewsDescend: Maybe<Descend>) {
  environment().selection = mapMaybe(
    maybe(bindMaybe(environment().selection, selection => descendFromCursor(rootDescend, viewsDescend, selection.cursor)),
      () => findTabStop(rootDescend, shift ? -1 : 1),
      descend => findNextTabStop(descend, shift ? -1 : 1) ),
    tabStop => ({cursor: tabStop.cursor}) )}

export function navKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Tab": {
      e.preventDefault()
      runE(() => doTab(e.shiftKey, rootDescend, viewsDescend))
      return true }
    case "Escape": {
      e.preventDefault()
      return runE(() => maybe(environment().selection, () => false, selection => { environment().selection = nothing; return true })) }}
  return false }

export function listKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case ",":
      return runE(() => booleanFromMaybe(selectionCursorBindMaybe(cursor => {
        let listInserter = (requireMeta: boolean) => () => (e.metaKey || !requireMeta)
          ? altMaybe(
            bindMaybe(insertAfterListElemCursor(cursor), cursor => {
              e.preventDefault()
              environment().selection = {cursor}
              return {} }),
            () => mapMaybe(appendToListCursor(cursor), cursor => {
              e.preventDefault()
              environment().selection = {cursor}
              return {} }))
          : nothing
        return bindMaybe(_get(cursor.parent, cursor.label), id => matchID(id, listInserter(false), listInserter(true), listInserter(false))) })))}
  return false }

export let defaultKeyHandler: KeyHandler = composedKeyHandler(deleteKeyHandler, navKeyHandler, arrowNavKeyHandler, listKeyHandler)