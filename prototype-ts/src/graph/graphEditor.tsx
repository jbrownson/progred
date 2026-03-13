import * as EL from "electron"
import * as FS from "fs"
import * as React from "react"
import * as ReactDOM from "react-dom"
import { groupBy } from "../lib/Array"
import { assert } from "../lib/assert"
import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, maybe2, maybeToArray, nothing } from "../lib/Maybe"
import { bradParamsFromJSON } from "./bradParamsFromJSON"
import { Cursor } from "./Cursor"
import { createD, Descend } from "./D"
import { DComponent } from "./DComponent"
import { defaultRender, tryFirst } from "./defaultRender"
import { deleteSelection } from "./deleteSelection"
import { ECallbacks, noopECallbacks, readOnlyECallbacks, undoRedoECallbacks } from "./ECallbacks"
import { _get, environment, Environment, get, guidFromSource, logSelection, set, withEnvironment } from "./Environment"
import { AppPlatform, BradParams, ctorField, GUIDRootViews, HasID, jsonFromID, LoadAWS, Module, PutAWS, rootField, rootViewsCtor, viewsField } from "./graph"
import { garbageCollectGUIDMap, GUIDMap } from "./GUIDMap"
import { generateGUID, guidFromID, ID, matchID, nidFromNumber, sidFromString } from "./ID"
import { jsonFromBradParams } from "./jsonFromBradParams"
import { jsonFromString } from "./jsonFromString"
import { defaultKeyHandler } from "./keyHandler"
import { libraries } from "./libraries/libraries"
import { load } from "./load"
import { loadAWSFromAppPlatform } from "./loadAWSFromAppPlatform"
import { putAWSFromAppPlatform } from "./putAWSFromAppPlatform"
import { putAWSSucceededFromPutAWS } from "./putAWSSucceededFromPutAWS"
import { dispatch } from "./R"
import { renderFromLibraries, renderFromModule } from "./renderFromLibraries"
import { renders } from "./renders"
import { save } from "./save"
import { _Selection } from "./Selection"
import { setCollapsed } from "./setCollapsed"
import { SparseSpanningTree } from "./SparseSpanningTree"
import { stringFromD } from "./stringFromD"
import { stringFromJSON } from "./stringFromJSON"
import { stringFromLoadAWS } from "./stringFromLoadAWS"
import { idFromStructure, structureForCursor } from "./structureForID"
import { UndoRedo } from "./UndoRedo"

const dialogOptions = {filters: [{name: "progred", extensions: ["progred"]}]}
const clipboardFormat = "progred_custom_clipboard_format"
const plainTextFormat = "text/plain"

EL.remote.Menu.setApplicationMenu(EL.remote.Menu.buildFromTemplate([{submenu: [{role: 'about'}, {role: 'quit'}]},
  { label: "File",
    submenu: [
      {label: "New", accelerator: "CmdOrCtrl+N", click: () => {
        undoStack = []
        redoStack = []
        guidRootViews = new GUIDRootViews(generateGUID())
        guidMap = new GUIDMap(new Map([[guidRootViews.id, new Map([[ctorField.id, rootViewsCtor.id]])]]))
        selection = {selection: nothing}
        filename = nothing
        rootComponent.forceUpdate() }},
      {label: "New View", click: () => rootComponent.runE(() => view(mapMaybe(environment().selection, selection => _get(selection.cursor.parent, selection.cursor.label))))},
      {label: "View Constructor", click: () => rootComponent.runE(() => bindMaybe(environment().selection, selection => bindMaybe(_get(selection.cursor.parent, selection.cursor.label), id => mapMaybe(_get(id, ctorField.id), view))))},
      {type: "separator"},
      {label: "Open…", accelerator: "CmdOrCtrl+O", click: () => {
        let filenames = EL.remote.dialog.showOpenDialog(dialogOptions)
        if (filenames) {
          filename = filenames[0]
          mapMaybe(filename, filename => loadJson(FS.readFileSync(filename, 'utf8'))) }}},
      {type: "separator"},
      {label: "Save", accelerator: "CmdOrCtrl+S", click: () => rootComponent.runE(() => maybe(filename, _saveAs, _save))},
      {label: "Save As…", accelerator: "CmdOrCtrl+Shift+S", click: () => rootComponent.runE(_saveAs)},
      {type: "separator"},
      {label: "Export Text…", accelerator: "CmdOrCtrl+Shift+T", click: () =>
        mapMaybe(EL.remote.dialog.showSaveDialog({}), filename => FS.writeFileSync(filename, stringFromD(rootComponent.rootDescend))) }]},
  { label: "Edit",
    submenu: [
      {label: "Undo", accelerator: "CmdOrCtrl+Z", click: () => {
        // if (actionIfTextInput("undo:")) return
        if (undoStack.length > 0) {
          rootComponent.runWithCustomCallbacks(() => {
            let actions = fromMaybe(undoStack.pop(), () => [])
            assert(actions.length > 0)
            actions.reverse().map(undoRedo => undoRedo.undo())
            actions.reverse()
            redoStack.push(actions) }, noopECallbacks) }}},
      {label: "Redo", accelerator: "Shift+CmdOrCtrl+Z", click: () => {
        // if (actionIfTextInput("redo:")) return
        if (redoStack.length > 0) {
          rootComponent.runWithCustomCallbacks(() => {
            let actions = fromMaybe(redoStack.pop(), () => [])
            assert(actions.length > 0)
            actions.map(undoRedo => undoRedo.redo())
            undoStack.push(actions) }, noopECallbacks) }}},
      {type: "separator"},
      {label: "Cut", accelerator: "CmdOrCtrl+X", click: () => {
        if (actionIfTextInputWithSelection("cut:")) return
        rootComponent.runE(() => { _copy(); deleteSelection(); environment().selection = nothing }) }},
      {label: "Copy", accelerator: "CmdOrCtrl+C", click: () => {
        if (actionIfTextInputWithSelection("copy:")) return
        rootComponent.runE(_copy) }},
      {label: "Paste Structure", accelerator: "CmdOrCtrl+Shift+V", click: () => rootComponent.runE(_pasteStructure) },
      {label: "Paste Reference", accelerator: "CmdOrCtrl+V", click: () => rootComponent.runE(_pasteID)},
      {label: "Select All", accelerator: "CmdOrCtrl+A", click: () => {
        if (!actionIfTextInput("selectAll:"))
          rootComponent.runE(() => environment().selection = {cursor: new Cursor(nothing, environment().rootViews.id, rootField.id, environment().sparseSpanningTree)}) }}]},
  { label: "Debug",
    submenu: [
      {label: "Refresh", accelerator: "CmdOrCtrl+R", click: () => EL.remote.BrowserWindow.getFocusedWindow().reload()},
      {label: "Open Dev Tools", accelerator: "CmdOrCtrl+Shift+I", click: () => EL.remote.getCurrentWebContents().openDevTools()},
      {label: "Console Log Selection", accelerator: "CmdOrCtrl+D", click: () => rootComponent.runE(() => logSelection())} ]},
  { label: "View",
    submenu: [
      {label: "Collapse", accelerator: "CmdOrCtrl+Shift+[", click: () => rootComponent.runE(() => bindMaybe(environment().selection, selection => setCollapsed(selection.cursor, true)))} ]},
  { label: "Transforms",
    submenu: [
      {label: "Load AWS -> Brad Params", click: () => transformAsync((id, f) => bindMaybe(LoadAWS.fromID(id), loadAWS => stringFromLoadAWS(loadAWS, hasSID =>
        f(() => bindMaybe(hasSID(), hasSID => bindMaybe(jsonFromString(hasSID), json => bradParamsFromJSON(json)))) )))},
      {label: "Brad Params -> string", click: () => transform(id => bindMaybe(BradParams.fromID(id), bradParams => bindMaybe(jsonFromBradParams(bradParams), stringFromJSON)))},
      {label: "App Platform -> LoadAWS", click: () => transform(id => bindMaybe(AppPlatform.fromID(id), appPlatform => loadAWSFromAppPlatform(appPlatform)))},
      {label: "App Platform -> PutAWS", click: () => transform(id => bindMaybe(AppPlatform.fromID(id), appPlatform => putAWSFromAppPlatform(appPlatform)))},
      {label: "Load AWS -> string", click: () => transformAsync((id, f) => bindMaybe(LoadAWS.fromID(id), loadAWS => stringFromLoadAWS(loadAWS, f)))},
      {label: "string -> JSON", click: () => transform(id => jsonFromString({id}))},
      {label: "JSON -> Brad Params", click: () => transform(id => bindMaybe(jsonFromID(id), bradParamsFromJSON))},
      {label: "Brad Params -> JSON", click: () => transform(id => bindMaybe(BradParams.fromID(id), jsonFromBradParams))},
      {label: "JSON -> string", click: () => transform(id => bindMaybe(jsonFromID(id), stringFromJSON))},
      {label: "Put AWS -> Put AWS Success", click: () => transformAsync((id, f) => bindMaybe(PutAWS.fromID(id), putAWS => putAWSSucceededFromPutAWS(putAWS, f)))} ]}]))

function view(id: Maybe<ID>) { let views = fromMaybe(environment().rootViews.views, () => []); environment().rootViews.setViews(maybe(id, () => views, id => [...views, {id}])) }

function transform(f: (id: ID) => Maybe<HasID>) {
  rootComponent.runE(() => bindMaybe(environment().selection, selection => bindMaybe(get(selection.cursor.parent, selection.cursor.label), ({id, source}) =>
    bindMaybe(guidFromSource(source), guid => bindMaybe(f(id), newID => set(guid, selection.cursor.label, newID.id))) )))}

function transformAsync(f: (id: ID, f: (f: () => Maybe<HasID>) => void) => void) {
  rootComponent.runE(() => bindMaybe(environment().selection, selection => bindMaybe(get(selection.cursor.parent, selection.cursor.label), ({id, source}) =>
    bindMaybe(guidFromSource(source), guid => f(id, f => rootComponent.runE(() => bindMaybe(f(), newID => set(guid, selection.cursor.label, newID.id))))) )))}

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
        EL.remote.Menu.sendActionToFirstResponder(action)
        return true }}
    if (document.activeElement instanceof HTMLTextAreaElement && document.activeElement.selectionStart !== document.activeElement.selectionEnd) {
      EL.remote.Menu.sendActionToFirstResponder(action)
      return true }}
  return false }

function actionIfTextInput(action: string) {
  if (document.activeElement) {
    if (document.activeElement instanceof HTMLInputElement) {
      let activeInputElement = document.activeElement as HTMLInputElement
      if (activeInputElement.type === "text") {
        EL.remote.Menu.sendActionToFirstResponder(action)
        return true }}
    if (document.activeElement instanceof HTMLTextAreaElement) {
      EL.remote.Menu.sendActionToFirstResponder(action)
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
          EL.remote.clipboard.writeBuffer(clipboardFormat, new Buffer(JSON.stringify(clip))) }
        catch(e) {} })))}

function _pasteID() {
  maybe2(environment().selection, idFromClipboardBuffer(EL.remote.clipboard.readBuffer(clipboardFormat)), () => {
    if (EL.remote.clipboard.availableFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      bindMaybe(environment().selection, selection => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, sidFromString(EL.remote.clipboard.readText())))) },
    (selection, id) => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, id)) )}

function _pasteStructure() {
  maybe2(environment().selection, structureIDFromClipboardBuffer(EL.remote.clipboard.readBuffer(clipboardFormat)), () => {
    if (EL.remote.clipboard.availableFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      bindMaybe(environment().selection, selection => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, sidFromString(EL.remote.clipboard.readText())))) },
      (selection, id) => mapMaybe(guidFromID(selection.cursor.parent), parent => set(parent, selection.cursor.label, id)) )}

function structureIDFromClipboardBuffer(buffer: Buffer): Maybe<ID> {
  try {
    let json = JSON.parse(buffer.toString())
    return idFromStructure(JSON.parse(json.structure)) }
  catch(e) {}
  return nothing }

function idFromClipboardBuffer(buffer: Buffer): Maybe<ID> {
  try {
    let json = JSON.parse(JSON.parse(buffer.toString()).id)
    return bindMaybe(json.string, jsonString => {
      if (typeof jsonString !== "string") return nothing
      switch (json.type) {
        case "guid": return jsonString
        case "number": let number = Number(jsonString); return number !== NaN ? nidFromNumber(number) : nothing
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
  FS.writeFileSync(filename, JSON.stringify(save({root: mapMaybe(e.rootViews.root, x => x.id), guidMap: maybe(e.rootViews.root, () => new GUIDMap, root => garbageCollectGUIDMap(e.guidMap, root.id))}), undefined, 2)) }

function _saveAs() { mapMaybe(EL.remote.dialog.showSaveDialog(dialogOptions), _filename => { filename = _filename; _save(filename) }) }

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
  render(): JSX.Element {
    let documentRender = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, defaultRender, readOnlyECallbacks().eCallbacks), () =>
      bindMaybe(bindMaybe(environment().rootViews.root, ({id}) => Module.fromID(id)), renderFromModule) )
    let {rootDescend, viewsDescend} = withEnvironment(new Environment(libraries, guidMap, guidRootViews, sparseSpanningTree, selection, tryFirst(dispatch(renders, libraryRender, ...maybeToArray(documentRender)), defaultRender), readOnlyECallbacks().eCallbacks), createD)
    this.rootDescend = rootDescend
    this.viewsDescend = viewsDescend
    return <div style={{position: "absolute", top: 0, left: 0, right: 0, bottom: 0}}>
      <div ref={leftPanel => this.leftPanel = leftPanel} className={maybe(viewsDescend, () => "", () => "leftPanel")}
        style={{display: "inline-block", width: maybe(viewsDescend, () => "100%", () => "60%"), height: "100%", overflow: "scroll"}}
        onScroll={() => { if (this.rootDComponent) this.rootDComponent.onScroll() }} >
        <div className="doc"><DComponent
          ref={dComponent => this.rootDComponent = dComponent}
          d={this.rootDescend}
          depth={0}
          scrollParent={() => this.leftPanel}
          runE={f => this.runE(f)} /></div></div>
      {maybe(this.viewsDescend, () => null, viewsDescend =>
        <div className="sidebar" style={{width: "40%", height: "100%", display: "inline-block"}}>
          <div className="separator" style={{height: "100%", display: "inline-block"}} />
          <div ref={rightPanel => this.rightPanel = rightPanel} className="rightPanel" style={{width: "100%", height: "100%", overflow: "scroll", display: "inline-block"}}
            onScroll={() => {if (this.viewsDComponent) this.viewsDComponent.onScroll()}} >
            <div className="views"><DComponent
              ref={dComponent => this.viewsDComponent = dComponent}
              d={viewsDescend}
              depth={0}
              scrollParent={() => this.rightPanel}
              runE={f => this.runE(f)} /></div></div></div>)}</div> }
  onScroll() { if(this.rootDComponent) this.rootDComponent.onScroll(); if (this.viewsDComponent) this.viewsDComponent.onScroll() }
  componentDidMount() { this.onScroll() }
  componentDidUpdate() { this.onScroll() } }

window.onclick = () => { rootComponent.runE(() => environment().selection = nothing) }
window.onkeydown = e => { defaultKeyHandler(e, rootComponent.rootDescend, rootComponent.viewsDescend, f => rootComponent.runE(f)) }

export let rootComponent = ReactDOM.render(<RootComponent />, document.getElementById('root') as HTMLElement) as RootComponent