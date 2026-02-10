import { assert } from "../lib/assert"
import { mapMaybe, Maybe, nothing } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { tryFirst } from "./defaultRender"
import { Entry } from "./Entry"
import { environment, get, SourceType } from "./Environment"
import { Match } from "./filters"
import { rootField, viewsField } from "./graph"
import { alwaysFail, Render } from "./R"
import { SelectionState, selectionStateFromCursor } from "./selectionIfSelected"

export type D = Block | Line | DText | DList | Descend | Label | Button | Placeholder | StringEditor | NumberEditor

export function matchD<A>(d: D, blockF: (block: Block) => A, lineF: (line: Line) => A, dTextF: (dText: DText) => A, dListF: (dList: DList) => A,
    descendF: (descend: Descend) => A, labelF: (label: Label) => A, buttonF: (button: Button) => A, placeholderF: (placeholder: Placeholder) => A,
    stringEditorF: (stringEditor: StringEditor) => A, numberEditorF: (numberEditor: NumberEditor) => A): A {
  return d instanceof Block ? blockF(d) : d instanceof Line ? lineF(d) : d instanceof DText ? dTextF(d) : d instanceof DList ? dListF(d) : d instanceof Descend ? descendF(d) :
    d instanceof Label ? labelF(d) : d instanceof Button ? buttonF(d) : d instanceof Placeholder ? placeholderF(d) : d instanceof StringEditor ? stringEditorF(d) : numberEditorF(d) }

export function isD<A>(a: A): Maybe<D> {
  return a instanceof Block || a instanceof Line || a instanceof DText || a instanceof DList || a instanceof Descend || a instanceof Label || a instanceof Button || a instanceof Placeholder || a instanceof StringEditor || a instanceof NumberEditor ? a : nothing }

export class Block {
  block() {} // These are a workaround this problem: https://github.com/Microsoft/TypeScript/issues/15615
  parent: Maybe<D>
  readonly children: D[]
  constructor(...children: D[]) { children.map(child => { assert(child.parent === nothing); child.parent = this }); this.children = children } }

export class Line {
  line() {}
  parent: Maybe<D>
  readonly children: D[]
  constructor(...children: D[]) { children.map(child => { assert(child.parent === nothing); child.parent = this }); this.children = children } }

export class DText {
  dText() {}
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public string: string) {} }

export class DList {
  dList() {}
  parent: Maybe<D>
  constructor(public opening: string, public children: D[], public closing: string, public separator: string, public clickBefore: (i: number) => void) {
    children.map(child => { assert(child.parent === nothing); child.parent = this }) }}

export class Descend {
  descend() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public child: D, public selectionState: Maybe<SelectionState>, public unmatching: boolean) { assert(child.parent === nothing); child.parent = this } }

export class Label {
  label() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public child: D) { assert(child.parent === nothing); child.parent = this } }

export class Button {
  button() {}
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public text: string, public action: () => void) {} }

export type PlaceholderState = {value?: string, itemSelection?: number, entryListAbove?: boolean}
export type PlaceholderSelectedState = {entries: (needle: string) => {a: Entry, matches: Match[]}[], placeholderState: PlaceholderState}
export class Placeholder {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public name: string, public selectedState: Maybe<PlaceholderSelectedState>) {} }

export type StringEditorSelectedState = {writable: boolean}
export class StringEditor {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public string: string, public stringEditorSelectedState: Maybe<StringEditorSelectedState>) {} }

export type NumberEditorState = {value?: string}
export type NumberEditorSelectedState = {writable: boolean, numberEditorState: NumberEditorState}
export class NumberEditor {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public number: number, public numberEditorSelectedState: Maybe<NumberEditorSelectedState>) {} }

export function createD(r: Render = alwaysFail) {
  let rootCursor = new Cursor(nothing, environment().rootViews.id, rootField.id, environment().sparseSpanningTree.map.get(rootField.id))
  let rootDescend = new Descend(rootCursor, tryFirst(r, environment().defaultRender)(rootCursor, mapMaybe(environment().rootViews.root, ({id}) =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().rootViews.id}}))), selectionStateFromCursor(rootCursor), false)
  let viewsCursor = new Cursor(nothing, environment().rootViews.id, viewsField.id, environment().sparseSpanningTree.map.get(viewsField.id))
  let viewsDescend = mapMaybe(get(environment().rootViews.id, viewsField.id), viewsSourceID => new Descend(viewsCursor, environment().defaultRender(viewsCursor, viewsSourceID), selectionStateFromCursor(viewsCursor), false))
  return {rootDescend, viewsDescend} }