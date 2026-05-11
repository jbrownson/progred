import { altMaybe, bindMaybe, booleanFromMaybe, firstMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { D, Descend } from "../render/D"
import { descendFromCursor } from "../cursor/descendFromCursor"
import { _get, environment } from "../Environment"
import { findNextTabStop, findTabStop } from "./findNextTabStop"
import { matchID } from "../model/ID"
import { editorCommandsForActiveElement } from "./EditorCommands"
import { appendToListCursor, insertAfterListElemCursor, selectionCursorBindMaybe } from "./listCursorActions"

export type KeyHandler = (e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, rootDescend, viewsDescend, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, rootDescend, viewsDescend, runE))) }

export function deleteKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Delete":
      return runE(() => maybe(editorCommandsForActiveElement(), () => false, commands =>
        maybe(commands.delete, () => false, delete_ => {
          e.stopPropagation()
          e.preventDefault()
          return delete_(rootDescend, viewsDescend, "forward") })))
    case "Backspace":
      return runE(() => maybe(editorCommandsForActiveElement(), () => false, commands =>
        maybe(commands.delete, () => false, delete_ => {
          e.stopPropagation()
          e.preventDefault()
          return delete_(rootDescend, viewsDescend, "backward") })))}
  return false }

function parentDescend(d: D): Maybe<Descend> {
  return bindMaybe(d.parent, parent => parent instanceof Descend ? parent : parentDescend(parent)) }

function goDown(d: D): Maybe<Descend> {
  return firstMaybe(d.children.map(child => (): Maybe<Descend> => altMaybe(child instanceof Descend ? child : nothing, () => goDown(child)))) }

function getSibling(d: D, n: number): Maybe<D> {
  return bindMaybe(d.parent, parent => bindMaybe(parent.children.findIndex(child => child === d), index => parent.children[index + n])) }

function goSibling(d: D, n: number): Maybe<Descend> {
  return altMaybe(
    bindMaybe(getSibling(d, n), sibling =>
      sibling instanceof Descend ? sibling : altMaybe(goDown(sibling), () => goSibling(sibling, n)) ),
    () => bindMaybe(d.parent, parent => goSibling(parent, n)) )}

function selectDescend(descend: Descend) { environment().selection = {cursor: descend.cursor} }

// TODO makeElementVisible
function keyboardNav(f: (descend: Descend) => Maybe<Descend>, rootDescend: Descend, viewsDescend: Maybe<Descend>): boolean {
  let env = environment()
  return booleanFromMaybe(mapMaybe(bindMaybe(bindMaybe(env.selection, selection => descendFromCursor(rootDescend, viewsDescend, selection.cursor)), f), d => { selectDescend(d); return {} })) }

export function arrowNavKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault()
      return runE(() => booleanFromMaybe(bindMaybe(environment().selection, selection =>
        mapMaybe(bindMaybe(descendFromCursor(rootDescend, viewsDescend, selection.cursor), parentDescend), selectDescend) )))
    case "ArrowRight":
      e.preventDefault()
      return runE(() => maybe(environment().selection,
        () => { selectDescend(rootDescend); return true },
        () => booleanFromMaybe(keyboardNav(goDown, rootDescend, viewsDescend)) ))
    case "ArrowDown":
      e.preventDefault()
      return runE(() => maybe(environment().selection,
        () => { selectDescend(rootDescend); return true },
        () => keyboardNav(d => goSibling(d, 1), rootDescend, viewsDescend) ))
    case "ArrowUp":
      e.preventDefault()
      return runE(() => keyboardNav(d => goSibling(d, -1), rootDescend, viewsDescend))}
  return false }

export function doTab(shift: boolean, rootDescend: Descend, viewsDescend: Maybe<Descend>): boolean {
  const selection = environment().selection
  const nextSelection = mapMaybe(
    maybe(bindMaybe(selection, selection => descendFromCursor(rootDescend, viewsDescend, selection.cursor)),
      () => findTabStop(rootDescend, shift ? -1 : 1),
      descend => findNextTabStop(descend, shift ? -1 : 1) ),
    tabStop => ({cursor: tabStop.cursor}) )
  environment().selection = fromMaybe(nextSelection, () => selection)
  return nextSelection !== nothing }

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
