import { altMaybe, bindMaybe, booleanFromMaybe, firstMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { D, Descend } from "../render/D"
import { Cursor } from "../cursor/Cursor"
import { descendFromCursor } from "../cursor/descendFromCursor"
import { findNextTabStop, findTabStop } from "./findNextTabStop"
import { commitToActiveElement, editorCommandsForActiveElement, editorKeyDownAction } from "./EditorCommands"
import { descendForActiveElement, focusEditorForDescend } from "./EditorFocus"

export type KeyHandler = (e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, rootDescend, viewsDescend, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, rootDescend, viewsDescend, runE))) }

export function deleteKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Delete":
      return runE(() => {
        let committed = commitToActiveElement(nothing)
        if (committed) {
          e.stopPropagation()
          e.preventDefault() }
        return committed })
    case "Backspace":
      return runE(() => {
        let committed = commitToActiveElement(nothing)
        if (committed) {
          e.stopPropagation()
          e.preventDefault() }
        return committed })}
  return false }

export function activeEditorKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  let keyDownAction = editorKeyDownAction(editorCommandsForActiveElement(), e)
  return maybe(keyDownAction, () => false, action => runE(() => {
    action()
    return true })) }

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

function dContains(d: D, target: D): boolean {
  return d === target || booleanFromMaybe(d.children.find(child => dContains(child, target))) }

function activeDescend(rootDescend: Descend, viewsDescend: Maybe<Descend>): Maybe<Descend> {
  let descend = descendForActiveElement()
  return descend && (dContains(rootDescend, descend) || maybe(viewsDescend, () => false, viewsDescend => dContains(viewsDescend, descend)))
    ? descend
    : nothing }

// TODO makeElementVisible
function keyboardNav(f: (descend: Descend) => Maybe<Descend>, rootDescend: Descend, viewsDescend: Maybe<Descend>): boolean {
  return booleanFromMaybe(mapMaybe(bindMaybe(activeDescend(rootDescend, viewsDescend), f), d => { focusEditorForDescend(d); return {} })) }

export function arrowNavKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault()
      return runE(() => booleanFromMaybe(mapMaybe(bindMaybe(activeDescend(rootDescend, viewsDescend), parentDescend), focusEditorForDescend) ))
    case "ArrowRight":
      e.preventDefault()
      return runE(() => maybe(activeDescend(rootDescend, viewsDescend),
        () => focusEditorForDescend(rootDescend),
        () => booleanFromMaybe(keyboardNav(goDown, rootDescend, viewsDescend)) ))
    case "ArrowDown":
      e.preventDefault()
      return runE(() => maybe(activeDescend(rootDescend, viewsDescend),
        () => focusEditorForDescend(rootDescend),
        () => keyboardNav(d => goSibling(d, 1), rootDescend, viewsDescend) ))
    case "ArrowUp":
      e.preventDefault()
      return runE(() => keyboardNav(d => goSibling(d, -1), rootDescend, viewsDescend))}
  return false }

export function doTab(shift: boolean, rootDescend: Descend, viewsDescend: Maybe<Descend>, cursor: Maybe<Cursor> = nothing): boolean {
  const descend = altMaybe(activeDescend(rootDescend, viewsDescend), () => bindMaybe(cursor, cursor => descendFromCursor(rootDescend, viewsDescend, cursor)))
  const nextDescend = maybe(
    maybe(descend,
      () => findTabStop(rootDescend, shift ? -1 : 1),
      descend => findNextTabStop(descend, shift ? -1 : 1) ),
    () => nothing,
    tabStop => tabStop)
  mapMaybe(nextDescend, focusEditorForDescend)
  return nextDescend !== nothing }

export function navKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Tab": {
      e.preventDefault()
      runE(() => doTab(e.shiftKey, rootDescend, viewsDescend))
      return true }
    case "Escape": {
      e.preventDefault()
      return false }}
  return false }

export let defaultKeyHandler: KeyHandler = composedKeyHandler(activeEditorKeyHandler, deleteKeyHandler, navKeyHandler, arrowNavKeyHandler)
