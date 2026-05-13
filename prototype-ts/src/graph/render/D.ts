import { assert } from "../../lib/assert"
import { mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { tryFirst } from "./defaultRender"
import { Entry } from "../editor/Entry"
import { Environment, environment, get, SourceType, withEnvironment } from "../Environment"
import { Match } from "../editor/filters"
import { rootField, viewsField } from "../graph"
import { alwaysFail, Render } from "./R"
import { GUID, ID, NID, SID } from "../model/ID"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { edgeContextFromCursor } from "../editor/edgeContextFromCursor"

export type D = Block | Line | DText | DIdenticon | DList | Descend | EditorBehavior | GuidEditor | SupportsUnderselection | Label | Collapsible | CollapseToggle | Button | PlaceholderEditor | StringEditor | NumberEditor

export function matchD<A>(d: D, blockF: (block: Block) => A, lineF: (line: Line) => A, dTextF: (dText: DText) => A, dIdenticonF: (dIdenticon: DIdenticon) => A, dListF: (dList: DList) => A,
    descendF: (descend: Descend) => A, editorBehaviorF: (editorBehavior: EditorBehavior) => A, guidEditorF: (guidEditor: GuidEditor) => A, supportsUnderselectionF: (supportsUnderselection: SupportsUnderselection) => A, labelF: (label: Label) => A, collapsibleF: (collapsible: Collapsible) => A, collapseToggleF: (collapseToggle: CollapseToggle) => A, buttonF: (button: Button) => A, placeholderEditorF: (placeholderEditor: PlaceholderEditor) => A,
    stringEditorF: (stringEditor: StringEditor) => A, numberEditorF: (numberEditor: NumberEditor) => A): A {
  return d instanceof Block ? blockF(d) : d instanceof Line ? lineF(d) : d instanceof DText ? dTextF(d) : d instanceof DIdenticon ? dIdenticonF(d) : d instanceof DList ? dListF(d) : d instanceof Descend ? descendF(d) :
    d instanceof EditorBehavior ? editorBehaviorF(d) : d instanceof GuidEditor ? guidEditorF(d) : d instanceof SupportsUnderselection ? supportsUnderselectionF(d) : d instanceof Label ? labelF(d) : d instanceof Collapsible ? collapsibleF(d) : d instanceof CollapseToggle ? collapseToggleF(d) : d instanceof Button ? buttonF(d) : d instanceof PlaceholderEditor ? placeholderEditorF(d) : d instanceof StringEditor ? stringEditorF(d) : numberEditorF(d) }

export function isD<A>(a: A): Maybe<D> {
  return a instanceof Block || a instanceof Line || a instanceof DText || a instanceof DIdenticon || a instanceof DList || a instanceof Descend || a instanceof EditorBehavior || a instanceof GuidEditor || a instanceof SupportsUnderselection || a instanceof Label || a instanceof Collapsible || a instanceof CollapseToggle || a instanceof Button || a instanceof PlaceholderEditor || a instanceof StringEditor || a instanceof NumberEditor ? a : nothing }

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

export class DIdenticon {
  dIdenticon() {}
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public guid: GUID, public size = 16) {} }

export type ListInsertionPoint = {
  entries: (needle: string) => {a: Entry, matches: Match[]}[],
  editorCommands: EditorCommands }

export class DList {
  dList() {}
  parent: Maybe<D>
  constructor(public opening: string, public children: D[], public closing: string, public separator: string, public collapseToggle: Maybe<CollapseToggle> = nothing, public insertionPoints: ListInsertionPoint[] = []) {
    children.map(child => { assert(child.parent === nothing); child.parent = this })
    if (collapseToggle) { assert(collapseToggle.parent === nothing); collapseToggle.parent = this } }}

export class Descend {
  descend() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public child: D, public unmatching: boolean, public edgeContext: EdgeContext = {}) { assert(child.parent === nothing); child.parent = this } }

export class EditorBehavior {
  editorBehavior() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public editorCommands: EditorCommands, public child: D) { assert(child.parent === nothing); child.parent = this } }

export class GuidEditor {
  guidEditor() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public id: GUID, public child: D, public focusWhenSelected: boolean, public editorCommands: EditorCommands) { assert(child.parent === nothing); child.parent = this } }

export class SupportsUnderselection {
  supportsUnderselection() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public id: ID, public child: D) { assert(child.parent === nothing); child.parent = this } }

export class Label {
  label() {}
  parent: Maybe<D>
  get children() { return [this.child] }
  constructor(public cursor: Cursor, public child: D) { assert(child.parent === nothing); child.parent = this } }

export class Collapsible {
  collapsible() {}
  parent: Maybe<D>
  environment: Environment
  get children() { return [this.child(this.defaultCollapsed, () => {})] }
  child(collapsed: boolean, setCollapsed: (collapsed: boolean) => void) {
    return withEnvironment(this.environment, () => {
      let child = this.render(collapsed, setCollapsed)
      assert(child.parent === nothing)
      child.parent = this
      return child }) }
  constructor(public defaultCollapsed: boolean, public singleLine: boolean, public render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D) {
    this.environment = environment() } }

export class CollapseToggle {
  collapseToggle() {}
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public collapsed: boolean, public action: () => void) {} }

export class Button {
  button() {}
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public text: string, public action: () => void) {} }

export type PlaceholderEditorState = {value?: string, itemSelection?: number, entryListAbove?: boolean, completionOpen?: boolean}
export type PlaceholderEditorActiveState = {entries: (needle: string) => {a: Entry, matches: Match[]}[], editorState: PlaceholderEditorState}
export class PlaceholderEditor {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public name: string, public entries: (needle: string) => {a: Entry, matches: Match[]}[], public activeState: Maybe<PlaceholderEditorActiveState>, public editorCommands: EditorCommands) {} }

export class StringEditor {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public id: SID, public string: string, public writable: boolean, public editorCommands: EditorCommands) {} }

export class NumberEditor {
  parent: Maybe<D>
  get children() { return [] as D[] }
  constructor(public id: NID, public number: number, public writable: boolean, public editorCommands: EditorCommands) {} }

export function createD(r: Render = alwaysFail) {
  let rootCursor = new Cursor(nothing, environment().rootViews.id, rootField.id)
  let rootDescend = new Descend(rootCursor, tryFirst(r, environment().defaultRender)(rootCursor, mapMaybe(environment().rootViews.root, ({id}) =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().rootViews.id}})), edgeContextFromCursor(rootCursor)), false, edgeContextFromCursor(rootCursor))
  let viewsCursor = new Cursor(nothing, environment().rootViews.id, viewsField.id)
  let viewsDescend = mapMaybe(get(environment().rootViews.id, viewsField.id), viewsSourceID => new Descend(viewsCursor, environment().defaultRender(viewsCursor, viewsSourceID, edgeContextFromCursor(viewsCursor)), false, edgeContextFromCursor(viewsCursor)))
  return {rootDescend, viewsDescend} }

export function supportsUnderselection(d: D): boolean {
  return d instanceof SupportsUnderselection || !(d instanceof Descend) && d.children.some(supportsUnderselection) }
