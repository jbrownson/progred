import * as React from "react"
import { createRoot } from "react-dom/client"
import { groupBy } from "../lib/Array"
import { assert } from "../lib/assert"
import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, maybe2, maybeToArray, nothing } from "../lib/Maybe"
import { bradParamsFromJSON } from "./transforms/bradParamsFromJSON"
import { Cursor } from "./cursor/Cursor"
import { createD, Descend } from "./render/D"
import { DComponent } from "./components/DComponent"
import { defaultRender, tryFirst } from "./render/defaultRender"
import { deleteSelection } from "./editor/deleteSelection"
import { ECallbacks, noopECallbacks, readOnlyECallbacks, undoRedoECallbacks } from "./editor/ECallbacks"
import { _get, environment, Environment, get, guidFromSource, logSelection, set, withEnvironment } from "./Environment"
import { BradParams, ctorField, GUIDRootViews, HasID, jsonFromID, Module, rootField, rootViewsCtor, viewsField } from "./graph"
import { garbageCollectGUIDMap, GUIDMap } from "./model/GUIDMap"
import { generateGUID, guidFromID, ID, matchID, nidFromNumber, sidFromString } from "./model/ID"
import { jsonFromBradParams } from "./transforms/jsonFromBradParams"
import { jsonFromString } from "./transforms/jsonFromString"
import { defaultKeyHandler } from "./editor/keyHandler"
import { libraries } from "./libraries/libraries"
import { load } from "./model/load"
import { dispatch } from "./render/R"
import { renderFromLibraries, renderFromModule } from "./render/renderFromLibraries"
import { renders } from "./render/renders"
import { save } from "./model/save"
import { _Selection } from "./editor/Selection"
import { setCollapsed } from "./editor/setCollapsed"
import { SparseSpanningTree } from "./SparseSpanningTree"
import { stringFromD } from "./transforms/stringFromD"
import { stringFromJSON } from "./transforms/stringFromJSON"
import { idFromStructure, structureForCursor } from "./structureForID"
import { UndoRedo } from "./editor/UndoRedo"

const progredFileFilters = [{name: "progred", extensions: ["progred"]}]
const clipboardFormat = "progred_custom_clipboard_format"
const plainTextFormat = "text/plain"
const progred = window.progred

function handleMenuAction(action: string) {
  switch (action) {
    case "new":
      undoStack = []
      redoStack = []
      guidRootViews = new GUIDRootViews(generateGUID())
      guidMap = new GUIDMap(new Map([[guidRootViews.id, new Map([[ctorField.id, rootViewsCtor.id]])]]))
      selection = {selection: nothing}
      filename = nothing
      rootComponent.forceUpdate()
      break
    case "new-view":
      rootComponent.runE(() => view(mapMaybe(environment().selection, selection => _get(selection.cursor.parent, selection.cursor.label))))
      break
    case "view-constructor":
      rootComponent.runE(() => bindMaybe(environment().selection, selection => bindMaybe(_get(selection.cursor.parent, selection.cursor.label), id => mapMaybe(_get(id, ctorField.id), view))))
      break
    case "open":
      void openDocument()
      break
    case "save":
      saveCurrent()
      break
    case "save-as":
      saveCurrentAs()
      break
    case "export-text":
      void progred.saveFileAs(stringFromD(rootComponent.rootDescend))
      break
    case "undo":
      undo()
      break
    case "redo":
      redo()
      break
    case "cut":
      if (actionIfTextInputWithSelection("cut:")) return
      rootComponent.runE(() => { _copy(); deleteSelection(); environment().selection = nothing })
      break
    case "copy":
      if (actionIfTextInputWithSelection("copy:")) return
      rootComponent.runE(_copy)
      break
    case "paste-structure":
      rootComponent.runE(_pasteStructure)
      break
    case "paste-reference":
      rootComponent.runE(_pasteID)
      break
    case "select-all":
      if (!actionIfTextInput("selectAll:"))
        rootComponent.runE(() => environment().selection = {cursor: new Cursor(nothing, environment().rootViews.id, rootField.id, environment().sparseSpanningTree)})
      break
    case "console-log-selection":
      rootComponent.runE(() => logSelection())
      break
    case "collapse":
      rootComponent.runE(() => bindMaybe(environment().selection, selection => setCollapsed(selection.cursor, true)))
      break
    case "transform-brad-params-string":
      transform(id => bindMaybe(BradParams.fromID(id), bradParams => bindMaybe(jsonFromBradParams(bradParams), stringFromJSON)))
      break
    case "transform-string-json":
      transform(id => jsonFromString({id}))
      break
    case "transform-json-brad-params":
      transform(id => bindMaybe(jsonFromID(id), bradParamsFromJSON))
      break
    case "transform-brad-params-json":
      transform(id => bindMaybe(BradParams.fromID(id), jsonFromBradParams))
      break
    case "transform-json-string":
      transform(id => bindMaybe(jsonFromID(id), stringFromJSON))
      break
  }
}

async function openDocument() {
  const file = await progred.openFile()
  if (!file) return
  filename = file.path
  loadJson(file.contents)
}

function undo() {
  if (undoStack.length > 0) {
    rootComponent.runWithCustomCallbacks(() => {
      let actions = fromMaybe(undoStack.pop(), () => [])
      assert(actions.length > 0)
      actions.reverse().map(undoRedo => undoRedo.undo())
      actions.reverse()
      redoStack.push(actions) }, noopECallbacks) }}

function redo() {
  if (redoStack.length > 0) {
    rootComponent.runWithCustomCallbacks(() => {
      let actions = fromMaybe(redoStack.pop(), () => [])
      assert(actions.length > 0)
      actions.map(undoRedo => undoRedo.redo())
      undoStack.push(actions) }, noopECallbacks) }}

function saveCurrent() {
  rootComponent.runE(() => maybe(filename, _saveAs, _save))
}

function saveCurrentAs() {
  rootComponent.runE(_saveAs)
}

function view(id: Maybe<ID>) { let views = fromMaybe(environment().rootViews.views, () => []); environment().rootViews.setViews(maybe(id, () => views, id => [...views, {id}])) }

function transform(f: (id: ID) => Maybe<HasID>) {
  rootComponent.runE(() => bindMaybe(environment().selection, selection => bindMaybe(get(selection.cursor.parent, selection.cursor.label), ({id, source}) =>
    bindMaybe(guidFromSource(source), guid => bindMaybe(f(id), newID => set(guid, selection.cursor.label, newID.id))) )))}

let undoStack: UndoRedo[][] = []
let redoStack: UndoRedo[][] = []
let sparseSpanningTree = new SparseSpanningTree(nothing, new Map([[rootField.id, new SparseSpanningTree], [viewsField.id, new SparseSpanningTree]]))
let guidRootViews = new GUIDRootViews(generateGUID())
let guidMap = new GUIDMap(new Map([[guidRootViews.id, new Map([[ctorField.id, rootViewsCtor.id]])]]))
let selection: {selection: Maybe<_Selection>} = {selection: nothing}
let filename: Maybe<string> = nothing

let libraryRender = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, tryFirst(renders, defaultRender), readOnlyECallbacks().eCallbacks), () => renderFromLibraries(libraries))

function actionIfTextInputWithSelection(action: string) {
  if (document.activeElement) {
    if (document.activeElement instanceof HTMLInputElement) {
      let activeInputElement = document.activeElement as HTMLInputElement
      if (activeInputElement.type === "text" && activeInputElement.selectionStart !== activeInputElement.selectionEnd) {
        progred.sendActionToFirstResponder(action)
        return true }}
    if (document.activeElement instanceof HTMLTextAreaElement && document.activeElement.selectionStart !== document.activeElement.selectionEnd) {
      progred.sendActionToFirstResponder(action)
      return true }}
  return false }

function actionIfTextInput(action: string) {
  if (document.activeElement) {
    if (document.activeElement instanceof HTMLInputElement) {
      let activeInputElement = document.activeElement as HTMLInputElement
      if (activeInputElement.type === "text") {
        progred.sendActionToFirstResponder(action)
        return true }}
    if (document.activeElement instanceof HTMLTextAreaElement) {
      progred.sendActionToFirstResponder(action)
      return true }}
  return false }

function _copy() {
  bindMaybe(environment().selection, selection =>
    bindMaybe(guidFromID(selection.cursor.parent), parent =>
      mapMaybe(_get(parent, selection.cursor.label), id => {
        try {
          const clip = {
            structure: clipboardStringForStructure(selection.cursor),
            id: clipboardStringForID(id) }
          progred.writeClipboardText(clipboardFormat, JSON.stringify(clip)) }
        catch(e) {} })))}

function _pasteID() {
  maybe2(environment().selection, idFromClipboardText(progred.readClipboardText(clipboardFormat)), () => {
    if (progred.availableClipboardFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      bindMaybe(environment().selection, selection => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, sidFromString(progred.readPlainText())))) },
    (selection, id) => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, id)) )}

function _pasteStructure() {
  maybe2(environment().selection, structureIDFromClipboardText(progred.readClipboardText(clipboardFormat)), () => {
    if (progred.availableClipboardFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      bindMaybe(environment().selection, selection => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, sidFromString(progred.readPlainText())))) },
      (selection, id) => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, id)) )}

function structureIDFromClipboardText(text: Maybe<string>): Maybe<ID> {
  try {
    let json = JSON.parse(fromMaybe(text, () => ""))
    return idFromStructure(JSON.parse(json.structure)) }
  catch(e) {}
  return nothing }

function idFromClipboardText(text: Maybe<string>): Maybe<ID> {
  try {
    let json = JSON.parse(JSON.parse(fromMaybe(text, () => "")).id)
    return bindMaybe(json.string, jsonString => {
      if (typeof jsonString !== "string") return nothing
      switch (json.type) {
        case "guid": return jsonString
        case "number": let number = Number(jsonString); return !Number.isNaN(number) ? nidFromNumber(number) : nothing
        case "string": sidFromString(jsonString) }})}
  catch(e) {}
  return nothing }

function clipboardStringForID(id: ID): string {
  return JSON.stringify(matchID<{type: string, string: string}>(id,
    guid => ({type: "guid", string: guid}),
    (sid, s) => ({type: "string", string: s}),
    nid => ({type: "number", string: String(nid)}) ))}

function clipboardStringForStructure(cursor: Cursor): string {
  return JSON.stringify(structureForCursor(cursor, rootComponent.rootDescend, rootComponent.viewsDescend)) }

function _save(filename: string) {
  let e = environment()
  void progred.writeFile(filename, JSON.stringify(save({root: mapMaybe(e.rootViews.root, x => x.id), guidMap: maybe(e.rootViews.root, () => new GUIDMap, root => garbageCollectGUIDMap(e.guidMap, root.id))}), undefined, 2)) }

function _saveAs() {
  let e = environment()
  const contents = JSON.stringify(save({root: mapMaybe(e.rootViews.root, x => x.id), guidMap: maybe(e.rootViews.root, () => new GUIDMap, root => garbageCollectGUIDMap(e.guidMap, root.id))}), undefined, 2)
  void progred.saveFileAs(contents, progredFileFilters).then(_filename => {
    if (_filename) filename = _filename
  }) }

function loadJson(json: string) {
  mapMaybe(mapMaybe(JSON.parse(json), load), ({guidMap: _guidMap, root: _root}) => {
    undoStack = []
    redoStack = []
    guidMap = _guidMap
    selection = {selection: nothing}
    guidRootViews = new GUIDRootViews(generateGUID())
    guidMap.set(guidRootViews.id, ctorField.id, rootViewsCtor.id)
    mapMaybe(_root, _root => guidMap.set(guidRootViews.id, rootField.id, _root))
    rootComponent.forceUpdate() })}

export class RootComponent extends React.Component<{}, {}> {
  rootDComponent: DComponent | null
  viewsDComponent: DComponent | null
  rootDescend: Descend
  viewsDescend: Maybe<Descend>
  inRunE = false
  leftPanel: HTMLElement | null
  rightPanel: HTMLElement | null
  runWithCustomCallbacks<A>(f: () => A, eCallbacks: ECallbacks) {
    assert(!this.inRunE)
    this.inRunE = true
    let a = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, tryFirst(renders, defaultRender), eCallbacks), f)
    this.forceUpdate() // TODO
    this.inRunE = false
    return a }
  runE<A>(f: () => A) {
    let {undoRedoArray, eCallbacks} = undoRedoECallbacks()
    let a = this.runWithCustomCallbacks(f, eCallbacks)
    if (undoRedoArray.length > 0) {
      const {trues, falses} = groupBy(undoRedoArray, undoRedo => undoRedo.selectionAction)
      if (falses.length !== 0) {
        undoStack.push(undoRedoArray) }
      else {
        const toInsert = trues[trues.length - 1]
        if (undoStack.length > 0) {
          let toModify = undoStack[undoStack.length - 1]
          toModify.push(toInsert) }
        if (redoStack.length > 0) {
          const toModify = redoStack[redoStack.length - 1]
          redoStack[redoStack.length - 1] = [new UndoRedo(toInsert.redo, toInsert.undo, true), ...toModify] }}}
    return a }
  render() {
    let documentRender = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, defaultRender, readOnlyECallbacks().eCallbacks), () =>
      bindMaybe(bindMaybe(environment().rootViews.root, ({id}) => Module.fromID(id)), renderFromModule) )
    let {rootDescend, viewsDescend} = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, tryFirst(dispatch(renders, libraryRender, ...maybeToArray(documentRender)), defaultRender), readOnlyECallbacks().eCallbacks), createD)
    this.rootDescend = rootDescend
    this.viewsDescend = viewsDescend
    return <div style={{position: "absolute", top: 0, left: 0, right: 0, bottom: 0}}>
      <div ref={leftPanel => { this.leftPanel = leftPanel }} className={maybe(viewsDescend, () => "", () => "leftPanel")}
        style={{display: "inline-block", width: maybe(viewsDescend, () => "100%", () => "60%"), height: "100%", overflow: "scroll"}}
        onScroll={() => { if (this.rootDComponent) this.rootDComponent.onScroll() }} >
        <div className="doc"><DComponent
          ref={dComponent => { this.rootDComponent = dComponent }}
          d={this.rootDescend}
          depth={0}
          scrollParent={() => this.leftPanel}
          runE={f => this.runE(f)} /></div></div>
      {maybe(this.viewsDescend, () => null, viewsDescend =>
        <div className="sidebar" style={{width: "40%", height: "100%", display: "inline-block"}}>
          <div className="separator" style={{height: "100%", display: "inline-block"}} />
          <div ref={rightPanel => { this.rightPanel = rightPanel }} className="rightPanel" style={{width: "100%", height: "100%", overflow: "scroll", display: "inline-block"}}
            onScroll={() => {if (this.viewsDComponent) this.viewsDComponent.onScroll()}} >
            <div className="views"><DComponent
              ref={dComponent => { this.viewsDComponent = dComponent }}
              d={viewsDescend}
              depth={0}
              scrollParent={() => this.rightPanel}
              runE={f => this.runE(f)} /></div></div></div>)}</div> }
  onScroll() { if(this.rootDComponent) this.rootDComponent.onScroll(); if (this.viewsDComponent) this.viewsDComponent.onScroll() }
  componentDidMount() { this.onScroll() }
  componentDidUpdate() { this.onScroll() } }

window.onclick = () => { if (rootComponent) rootComponent.runE(() => environment().selection = nothing) }
window.onkeydown = e => { if (rootComponent) defaultKeyHandler(e, rootComponent.rootDescend, rootComponent.viewsDescend, f => rootComponent.runE(f)) }
progred.onMenuAction(action => { if (rootComponent) handleMenuAction(action) })

export let rootComponent: RootComponent
createRoot(document.getElementById('root') as HTMLElement)
  .render(<RootComponent ref={component => { if (component) rootComponent = component }} />)
