import { mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import type { ID } from "../model/ID"
import type { Type } from "../graph"
import type { CopyResult } from "./Copy"

export type Commit = (id: Maybe<ID>) => void
export type EdgeContext = {
  commit?: Commit
  expectedType?: Maybe<Type>
}
export type EditorKeyDownEvent = {
  key: string,
  metaKey: boolean,
  ctrlKey: boolean,
  altKey: boolean,
  shiftKey: boolean,
  target: EventTarget,
  preventDefault: () => void,
  stopPropagation: () => void,
  commit?: Commit }

export type EditorCommands = {
  copy?: () => {referenceID: ID, copyResult: CopyResult}
  commit?: Commit
  newEdge?: () => void
  keyDown?: (e: EditorKeyDownEvent) => Maybe<() => void>
}

type KeyDownEvent = {
  key: string,
  metaKey: boolean,
  ctrlKey: boolean,
  altKey: boolean,
  shiftKey: boolean,
  target: EventTarget,
  preventDefault: () => void,
  stopPropagation: () => void }

const editorCommandsKey = Symbol("editorCommands")

type EditorCommandsElement = HTMLElement & {[editorCommandsKey]?: EditorCommands}

export function attachEditorCommands(element: HTMLElement, commands: EditorCommands) {
  (element as EditorCommandsElement)[editorCommandsKey] = commands
}

export function detachEditorCommands(element: HTMLElement) {
  delete (element as EditorCommandsElement)[editorCommandsKey]
}

export function editorCommandsForActiveElement(): Maybe<EditorCommands> {
  let element = document.activeElement
  return element instanceof HTMLElement ? (element as EditorCommandsElement)[editorCommandsKey] : nothing
}

export function editorKeyDownAction(commands: Maybe<EditorCommands>, e: KeyDownEvent): Maybe<() => void> {
  return maybe(commands, () => nothing, commands => mapMaybe(commands.keyDown, keyDown => keyDown({
    key: e.key,
    metaKey: e.metaKey,
    ctrlKey: e.ctrlKey,
    altKey: e.altKey,
    shiftKey: e.shiftKey,
    target: e.target,
    preventDefault: () => e.preventDefault(),
    stopPropagation: () => e.stopPropagation(),
    commit: commands.commit }))) }

export function commitToActiveElement(id: Maybe<ID>): boolean {
  return maybe(editorCommandsForActiveElement(), () => false, commands =>
    maybe(commands.commit, () => false, commit => {
      commit(id)
      return true })) }

export function commitIDToActiveElement(id: ID): boolean { return commitToActiveElement(id) }
