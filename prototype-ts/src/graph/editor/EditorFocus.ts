import { altMaybe, bindMaybe, firstMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { Edge } from "../model/Edge"
import type { EditorDescend } from "../render/DContext"
import { focus } from "./domFocus"

const editorFocusKey = Symbol("editorFocus")
const editorDescendKey = Symbol("editorDescend")
type PendingFocus =
  | {kind: "first"}
  | {kind: "parentDescendPath", path: number[]}
  | {kind: "nextTabStopFromDescendPath", path: number[], shift: boolean}
  | {kind: "nextTabStopFromDescendChildPath", path: number[], index: number}
let pendingFocus: Maybe<PendingFocus> = nothing

type EditorFocus = {
  edge?: Edge
  descend?: EditorDescend
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
  for (let current: Maybe<Element> = element; current instanceof HTMLElement; current = current.parentElement || nothing)
    if (editorDescendForElement(current) === descend) return current
  return nothing }

function childDescendElements(element: Maybe<HTMLElement>): HTMLElement[] {
  return editorDescendElements(element || document).filter(descendElement => parentDescendElement(descendElement) === element) }

function rootDescendElements(root: ParentNode): HTMLElement[] {
  return editorDescendElements(root).filter(descendElement => maybe(parentDescendElement(descendElement), () => true, parent => !(root instanceof Node && root.contains(parent)))) }

function childDescendElementsIn(root: ParentNode, element: Maybe<HTMLElement>): HTMLElement[] {
  return maybe(element, () => rootDescendElements(root), childDescendElements) }

function activeEditorDescendElement() {
  let activeDescend = editorFocusForActiveElement()?.descend
  return activeDescend && document.activeElement
    ? descendElementForDescend(document.activeElement, activeDescend)
    : document.activeElement instanceof HTMLElement ? parentDescendElement(document.activeElement) : nothing }

function focusEditorForDescendElement(descendElement: Maybe<HTMLElement>): boolean {
  return maybe(descendElement, () => false, descendElement => maybe(editorDescendForElement(descendElement), () => false, descend => {
    let element = editorFocusElements(descendElement).find(element => editorFocusForElement(element)?.descend === descend)
    return element ? focusElement(element) : maybe(editorFocusElements(descendElement)[0], () => false, focusElement) })) }

function focusElement(element: HTMLElement): boolean {
  let editorFocus = editorFocusForElement(element)
  return maybe(editorFocus, () => false, editorFocus => {
    pendingFocus = nothing
    focus(element)
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

function editorDescendPath(descendElement: HTMLElement): Maybe<number[]> {
  let parent = parentDescendElement(descendElement)
  let index = childDescendElements(parent).findIndex(child => child === descendElement)
  if (index < 0) return nothing
  return maybe(parent, () => [index], parent => mapMaybe(editorDescendPath(parent), path => [...path, index])) }

function descendElementFromPath(root: ParentNode, path: number[]): Maybe<HTMLElement> {
  let descendElement: Maybe<HTMLElement> = nothing
  for (let index of path) {
    let children = childDescendElementsIn(root, descendElement)
    let next = children[index]
    if (!next) return nothing
    descendElement = next }
  return descendElement }

function editorElementForElement(element: Element): Maybe<HTMLElement> {
  for (let current: Maybe<Element> = element; current instanceof HTMLElement; current = current.parentElement || nothing)
    if (editorFocusForElement(current) !== nothing) return current
  return nothing }

function nextEditorElementAfter(element: Element): Maybe<HTMLElement> {
  return editorFocusElements(document).find(editor =>
    element.compareDocumentPosition(editor) & Node.DOCUMENT_POSITION_FOLLOWING) }

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
  return editorFocusForElement(document.activeElement || nothing)
}

export function focusEditorFromElement(element: Element): boolean {
  return maybe(editorElementForElement(element), () => maybe(nextEditorElementAfter(element), () => false, focusElement), focusElement)
}

export function requestFocusFirstEditor() {
  pendingFocus = {kind: "first"}
}

export function requestFocusParentFromActiveElement() {
  pendingFocus = maybe(bindMaybe(activeEditorDescendElement(), parentDescendElement),
    (): PendingFocus => ({kind: "first"}),
    parent => maybe(editorDescendPath(parent),
      (): PendingFocus => ({kind: "first"}),
      (path): PendingFocus => ({kind: "parentDescendPath", path}))) }

export function requestNextTabStopFromActiveElement(shift = false) {
  pendingFocus = maybe(activeEditorDescendElement(),
    (): PendingFocus => ({kind: "first"}),
    descendElement => maybe(editorDescendPath(descendElement),
      (): PendingFocus => ({kind: "first"}),
      (path): PendingFocus => ({kind: "nextTabStopFromDescendPath", path, shift}))) }

export function requestNextTabStopFromDescendChildFromActiveElement(index: number) {
  pendingFocus = maybe(activeEditorDescendElement(),
    (): PendingFocus => ({kind: "first"}),
    descendElement => maybe(editorDescendPath(descendElement),
      (): PendingFocus => ({kind: "first"}),
      (path): PendingFocus => ({kind: "nextTabStopFromDescendChildPath", path, index}))) }

export function focusFirstEditor(root: ParentNode = document): boolean {
  return focusEditorForDescendElement(rootDescendElements(root)[0])
}

function focusDescendChild(descendElement: HTMLElement, index: number): boolean {
  return focusEditorForDescendElement(childDescendElements(descendElement)[index])
}

export function focusPendingEditor(root: HTMLElement): boolean {
  return maybe(pendingFocus, () => false, pendingFocus => {
    switch (pendingFocus.kind) {
      case "first":
        return focusFirstEditor(root)
      case "parentDescendPath":
        return maybe(descendElementFromPath(root, pendingFocus.path), () => focusFirstEditor(root), focusEditorForDescendElement)
      case "nextTabStopFromDescendPath":
        return maybe(descendElementFromPath(root, pendingFocus.path), () => focusFirstEditor(root), descendElement =>
          focusEditorForDescendElement(altMaybe(nextTabStop(descendElement, pendingFocus.shift ? -1 : 1), () => descendElement)))
      case "nextTabStopFromDescendChildPath":
        return maybe(descendElementFromPath(root, pendingFocus.path), () => focusFirstEditor(root), descendElement =>
          maybe(childDescendElements(descendElement)[pendingFocus.index], () => false, child =>
            focusEditorForDescendElement(altMaybe(nextTabStop(child, 1), () => child)))) }})
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

export function focusNextTabStop(shift: boolean): boolean {
  return focusEditorForDescendElement(nextTabStop(activeEditorDescendElement(), shift ? -1 : 1))
}
