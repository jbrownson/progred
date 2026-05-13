import { altMaybe, bindMaybe, firstMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { cursorsEqual } from "../cursor/Cursor"
import type { EditorDescend } from "../render/ProjectionContext"
import { focus } from "./ignoreFocusEvents"

const editorFocusKey = Symbol("editorFocus")
const editorDescendKey = Symbol("editorDescend")
type PendingFocus = {kind: "cursor", cursor: Cursor} | {kind: "nextTabStopFromCursor", cursor: Cursor, shift: boolean}
let pendingFocus: Maybe<PendingFocus> = nothing

type EditorFocus = {
  cursor: Cursor
  descend?: EditorDescend
  activate?: () => void
  focusWhenSelected?: boolean
  tabStop?: boolean
}

type EditorFocusElement = HTMLElement & {[editorFocusKey]?: EditorFocus}
type EditorDescendElement = HTMLElement & {[editorDescendKey]?: EditorDescend}

function editorFocusForElement(element: Maybe<Element>): Maybe<EditorFocus> {
  return element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey] : nothing
}

function editorDescendForElement(element: Maybe<Element>): Maybe<EditorDescend> {
  return element instanceof HTMLElement ? (element as EditorDescendElement)[editorDescendKey] : nothing
}

function editorFocusElements(root: ParentNode = document): HTMLElement[] {
  let elements = root instanceof HTMLElement ? [root, ...Array.from(root.querySelectorAll("*"))] : Array.from(root.querySelectorAll("*"))
  return elements.filter(element => {
    let editorFocus = editorFocusForElement(element)
    return element instanceof HTMLElement && editorFocus !== nothing && editorFocus.focusWhenSelected !== false }) as HTMLElement[] }

function editorDescendElements(root: ParentNode = document): HTMLElement[] {
  let elements = root instanceof HTMLElement ? [root, ...Array.from(root.querySelectorAll("*"))] : Array.from(root.querySelectorAll("*"))
  return elements.filter(element => element instanceof HTMLElement && editorDescendForElement(element) !== nothing) as HTMLElement[] }

function parentDescendElement(element: HTMLElement): Maybe<HTMLElement> {
  for (let parent = element.parentElement; parent; parent = parent.parentElement)
    if (editorDescendForElement(parent) !== nothing) return parent
  return nothing }

function descendElementForDescend(element: Element, descend: EditorDescend): Maybe<HTMLElement> {
  for (let current: Maybe<Element> = element; current instanceof HTMLElement; current = current.parentElement)
    if (editorDescendForElement(current) === descend) return current
  return nothing }

function childDescendElements(element: Maybe<HTMLElement>): HTMLElement[] {
  return editorDescendElements(element || document).filter(descendElement => parentDescendElement(descendElement) === element) }

function activeEditorDescendElement() {
  let activeDescend = editorFocusForActiveElement()?.descend
  return activeDescend && document.activeElement ? descendElementForDescend(document.activeElement, activeDescend) : nothing }

function focusEditorForDescendElement(descendElement: Maybe<HTMLElement>): boolean {
  return maybe(descendElement, () => false, descendElement => maybe(editorDescendForElement(descendElement), () => false, descend => {
    let element = editorFocusElements(descendElement).find(element => editorFocusForElement(element)?.descend === descend)
    return element ? focusElement(element) : focusEditorForCursor(descendElement, descend.cursor) })) }

function focusElement(element: HTMLElement): boolean {
  let editorFocus = editorFocusForElement(element)
  return maybe(editorFocus, () => false, editorFocus => {
    pendingFocus = nothing
    focus(element)
    editorFocus.activate?.()
    return true }) }

function sibling(descendElement: HTMLElement, n: number): Maybe<HTMLElement> {
  let siblings = childDescendElements(parentDescendElement(descendElement))
  return bindMaybe(siblings.findIndex(sibling => sibling === descendElement), index => siblings[index + n]) }

function firstChild(descendElement: HTMLElement): Maybe<HTMLElement> { return childDescendElements(descendElement)[0] }

function siblingOrAncestorSibling(descendElement: HTMLElement, n: number): Maybe<HTMLElement> {
  return altMaybe(sibling(descendElement, n), () => bindMaybe(parentDescendElement(descendElement), parent => siblingOrAncestorSibling(parent, n))) }

function descendHasTabStop(descendElement: HTMLElement): boolean {
  return editorFocusElements(descendElement).some(element => {
    let descend = editorDescendForElement(descendElement)
    let editorFocus = editorFocusForElement(element)
    return editorFocus !== nothing && editorFocus.descend === descend && editorFocus.tabStop }) }

function tabStopDown(descendElement: HTMLElement, n: number): Maybe<HTMLElement> {
  let children = childDescendElements(descendElement)
  return descendHasTabStop(descendElement) ? descendElement : firstMaybe((n > 0 ? children : children.reverse()).map(child => () => tabStopDown(child, n))) }

function tabStopDownChildren(descendElement: HTMLElement, n: number): Maybe<HTMLElement> {
  let children = childDescendElements(descendElement)
  return firstMaybe((n > 0 ? children : children.reverse()).map(child => () => tabStopDown(child, n))) }

function tabStopUp(descendElement: HTMLElement, n: number): Maybe<HTMLElement> {
  return altMaybe(bindMaybe(sibling(descendElement, n), sibling => tabStopDown(sibling, n)), () => bindMaybe(parentDescendElement(descendElement), parent => tabStopUp(parent, n))) }

function firstTabStop(n: number): Maybe<HTMLElement> {
  let roots = childDescendElements(nothing)
  return firstMaybe((n > 0 ? roots : roots.reverse()).map(root => () => tabStopDown(root, n))) }

function nextTabStop(descendElement: Maybe<HTMLElement>, n: number): Maybe<HTMLElement> {
  return maybe(descendElement, () => firstTabStop(n), descendElement => altMaybe(tabStopDownChildren(descendElement, n), () => tabStopUp(descendElement, n))) }

function descendElementForCursor(root: ParentNode, cursor: Cursor): Maybe<HTMLElement> {
  return editorDescendElements(root).find(descendElement => maybe(editorDescendForElement(descendElement), () => false, descend => cursorsEqual(descend.cursor, cursor))) }

function editorElementForCursor(root: ParentNode, cursor: Cursor): Maybe<HTMLElement> {
  return editorFocusElements(root).find(element => maybe(editorFocusForElement(element), () => false, editorFocus => cursorsEqual(editorFocus.cursor, cursor))) }

export function attachEditorFocus(element: HTMLElement, focus: EditorFocus) {
  (element as EditorFocusElement)[editorFocusKey] = focus
}

export function detachEditorFocus(element: HTMLElement) {
  delete (element as EditorFocusElement)[editorFocusKey]
}

export function attachEditorDescend(element: HTMLElement, descend: EditorDescend) {
  (element as EditorDescendElement)[editorDescendKey] = descend
}

export function editorFocusForActiveElement(): Maybe<EditorFocus> {
  return editorFocusForElement(document.activeElement)
}

export function focusEditorForCursor(root: HTMLElement, cursor: Cursor): boolean {
  return maybe(editorElementForCursor(root, cursor), () => false, focusElement)
}

export function requestFocusForCursor(cursor: Cursor) {
  pendingFocus = {kind: "cursor", cursor}
}

export function requestNextTabStopFromCursor(cursor: Cursor, shift = false) {
  pendingFocus = {kind: "nextTabStopFromCursor", cursor, shift}
}

export function focusPendingEditor(root: HTMLElement): boolean {
  return maybe(pendingFocus, () => false, pendingFocus => {
    switch (pendingFocus.kind) {
      case "cursor":
        return focusEditorForCursor(root, pendingFocus.cursor)
      case "nextTabStopFromCursor":
        return maybe(descendElementForCursor(root, pendingFocus.cursor), () => false, descendElement =>
          focusEditorForDescendElement(altMaybe(nextTabStop(descendElement, pendingFocus.shift ? -1 : 1), () => descendElement))) }})
}

export function focusParentEditor(): boolean {
  return focusEditorForDescendElement(bindMaybe(activeEditorDescendElement(), parentDescendElement))
}

export function focusChildEditor(): boolean {
  return focusEditorForDescendElement(bindMaybe(activeEditorDescendElement(), firstChild))
}

export function focusSiblingEditor(n: number): boolean {
  return focusEditorForDescendElement(bindMaybe(activeEditorDescendElement(), descendElement => siblingOrAncestorSibling(descendElement, n)))
}

export function focusFirstEditor(): boolean {
  return focusEditorForDescendElement(childDescendElements(nothing)[0])
}

export function focusNextTabStop(shift: boolean): boolean {
  return focusEditorForDescendElement(nextTabStop(activeEditorDescendElement(), shift ? -1 : 1))
}
