import * as React from "react"
import { createRoot } from "react-dom/client"
import { assert } from "../lib/assert"
import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, maybeToArray, nothing } from "../lib/Maybe"
import { bradParamsFromJSON } from "./transforms/bradParamsFromJSON"
import { Cursor } from "./cursor/Cursor"
import { createD, Descend } from "./render/D"
import { DComponent } from "./components/DComponent"
import { GraphViewComponent } from "./components/GraphViewComponent"
import { defaultRender, tryFirst } from "./render/defaultRender"
import { clipboardFormat, clipboardStringForCopyResult, copyIDFromClipboardText, idFromClipboardText, plainTextFormat } from "./editor/Clipboard"
import { composeECallbacks, ECallbacks, noopECallbacks, readOnlyECallbacks, undoRedoECallbacks } from "./editor/ECallbacks"
import { commitIDToActiveElement, commitToActiveElement, editorCommandsForActiveElement } from "./editor/EditorCommands"
import { editorFocusForActiveElement, focusEditorForCursor, focusPendingEditor, requestFocusForCursor } from "./editor/EditorFocus"
import { _delete, _get, environment, Environment, get, guidFromSource, logID, set, withEnvironment } from "./Environment"
import { BradParams, ctorField, GUIDRootViews, HasID, jsonFromID, Module, rootField, rootViewsCtor, viewsField } from "./graph"
import { garbageCollectGUIDMap, GUIDMap } from "./model/GUIDMap"
import { EdgeRef } from "./model/EdgeRef"
import { generateGUID, guidFromID, ID, sidFromString } from "./model/ID"
import { jsonFromBradParams } from "./transforms/jsonFromBradParams"
import { jsonFromString } from "./transforms/jsonFromString"
import { composedKeyHandler, defaultKeyHandler, KeyHandler } from "./editor/keyHandler"
import { libraries } from "./libraries/libraries"
import { load } from "./model/load"
import { dispatch } from "./render/R"
import { renderFromLibraries, renderFromModule } from "./render/renderFromLibraries"
import { renders } from "./render/renders"
import { save } from "./model/save"
import { buildGraphViewSnapshot, GraphSelection } from "./graphView/GraphViewSnapshot"
import { stringFromD } from "./transforms/stringFromD"
import { stringFromJSON } from "./transforms/stringFromJSON"
import { UndoRedo } from "./editor/UndoRedo"

const progredFileFilters = [{name: "progred", extensions: ["progred"]}]
const progred = window.progred

function handleMenuAction(action: string) {
  switch (action) {
    case "new":
      undoStack = []
      redoStack = []
      guidRootViews = new GUIDRootViews(generateGUID())
      guidMap = new GUIDMap(new Map([[guidRootViews.id, new Map([[ctorField.id, rootViewsCtor.id]])]]))
      initialFocusCursor = nothing
      rootComponent.initialFocusConsumed = false
      graphHighlight = nothing
      filename = nothing
      rootComponent.forceUpdate()
      break
    case "new-view":
      rootComponent.runE(() => view(activeID()))
      break
    case "view-constructor":
      rootComponent.runE(() => bindMaybe(activeID(), id => mapMaybe(_get(id, ctorField.id), view)))
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
    case "new-node":
      rootComponent.runE(newNode)
      break
    case "new-edge":
      rootComponent.runE(startNewEdge)
      break
    case "cut":
      if (actionIfTextInputWithSelection("cut:")) return
      rootComponent.runE(() => { _copy(); commitToActiveElement(nothing) })
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
    case "delete":
      rootComponent.runE(deleteActiveSelection)
      break
    case "select-all":
      if (!actionIfTextInput("selectAll:"))
        rootComponent.runE(() => requestFocusForCursor(new Cursor(nothing, environment().rootViews.id, rootField.id)))
      break
    case "console-log-selection":
      rootComponent.runE(() => mapMaybe(activeID(), logID))
      break
    case "toggle-graph":
      rootComponent.showGraph = !rootComponent.showGraph
      rootComponent.forceUpdate()
      break
    case "collapse":
      mapMaybe(editorCommandsForActiveElement()?.collapse, collapse => collapse())
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

function activeCursor(): Maybe<Cursor> { return editorFocusForActiveElement()?.cursor }

function activeEdge(): Maybe<EdgeRef> {
  return mapMaybe(activeCursor(), cursor => ({parent: cursor.parent, label: cursor.label})) }

function activeID(): Maybe<ID> {
  return bindMaybe(activeEdge(), edge => _get(edge.parent, edge.label)) }

function newNode() {
  const id = generateGUID()
  if (!commitIDToActiveElement(id))
    set(environment().rootViews.id, rootField.id, id) }

function startNewEdge() {
  mapMaybe(editorCommandsForActiveElement(), commands => mapMaybe(commands.newEdge, newEdge => newEdge())) }

function deleteActiveSelection(): boolean {
  return graphHighlight !== nothing
    ? deleteGraphSelection()
    : commitToActiveElement(nothing) }

function deleteGraphSelection(): boolean {
  return maybe(graphHighlight, () => false, graphSelection => {
    let deleted = false
    switch (graphSelection.kind) {
      case "edge":
        if (environment().guidMap.edges(graphSelection.source)?.has(graphSelection.label)) {
          _delete(graphSelection.source, graphSelection.label)
          deleted = true }
        break
      case "node":
        mapMaybe(guidFromID(graphSelection.id), guid =>
          mapMaybe(environment().guidMap.edges(guid), edges =>
            Array.from(edges.keys()).forEach(label => {
              _delete(guid, label)
              deleted = true })))
        for (let [source, edges] of Array.from(environment().guidMap.map)) {
          for (let [label, target] of Array.from(edges)) {
            if (target === graphSelection.id) {
              _delete(source, label)
              deleted = true }}}}
    if (deleted) graphHighlight = nothing
    return deleted }) }

function clearGraphHighlightCallbacks(eCallbacks: ECallbacks): ECallbacks {
  return composeECallbacks(eCallbacks, {...noopECallbacks, willSet: () => { graphHighlight = nothing }, willDelete: () => { graphHighlight = nothing }}) }

const graphKeyHandler: KeyHandler = (e, _rootDescend, _viewsDescend, runE) => {
  switch (e.key) {
    case "Delete":
    case "Backspace":
      if (graphHighlight === nothing) return false
      e.stopPropagation()
      e.preventDefault()
      return runE(deleteGraphSelection) }
  return false }

const keyHandler = composedKeyHandler(graphKeyHandler, defaultKeyHandler)

function transform(f: (id: ID) => Maybe<HasID>) {
  rootComponent.runE(() => bindMaybe(activeEdge(), edge => bindMaybe(get(edge.parent, edge.label), ({id, source}) =>
    bindMaybe(guidFromSource(source), guid => bindMaybe(f(id), newID => set(guid, edge.label, newID.id))) )))}

let undoStack: UndoRedo[][] = []
let redoStack: UndoRedo[][] = []
let guidRootViews = new GUIDRootViews(generateGUID())
let guidMap = new GUIDMap(new Map([[guidRootViews.id, new Map([[ctorField.id, rootViewsCtor.id]])]]))
let initialFocusCursor: Maybe<Cursor> = new Cursor(nothing, guidRootViews.id, rootField.id)
let graphHighlight: Maybe<GraphSelection> = nothing
let filename: Maybe<string> = nothing

let libraryRender = withEnvironment(new Environment(libraries, guidMap, guidRootViews, tryFirst(renders, defaultRender), readOnlyECallbacks().eCallbacks), () => renderFromLibraries(libraries))

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
  bindMaybe(editorCommandsForActiveElement(), commands =>
    mapMaybe(commands.copy, copy => {
      const {referenceID, copyResult} = copy()
      progred.writeClipboardText(clipboardFormat, clipboardStringForCopyResult(referenceID, copyResult)) }))}

function _pasteID() {
  maybe(idFromClipboardText(progred.readClipboardText(clipboardFormat)), () => {
    if (progred.availableClipboardFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      commitIDToActiveElement(sidFromString(progred.readPlainText())) },
    id => { commitIDToActiveElement(id) }) }

function _pasteStructure() {
  maybe(copyIDFromClipboardText(progred.readClipboardText(clipboardFormat)), () => {
    if (progred.availableClipboardFormats().indexOf(plainTextFormat) >= 0 && !actionIfTextInput("paste:"))
      commitIDToActiveElement(sidFromString(progred.readPlainText())) },
      id => { commitIDToActiveElement(id) }) }

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
    initialFocusCursor = nothing
    rootComponent.initialFocusConsumed = false
    graphHighlight = nothing
    guidRootViews = new GUIDRootViews(generateGUID())
    guidMap.set(guidRootViews.id, ctorField.id, rootViewsCtor.id)
    mapMaybe(_root, _root => guidMap.set(guidRootViews.id, rootField.id, _root))
    rootComponent.forceUpdate() })}

export class RootComponent extends React.Component<{}, {}> {
  rootDComponent: DComponent | null
  viewsDComponent: DComponent | null
  rootDescend: Descend
  viewsDescend: Maybe<Descend>
  showGraph = false
  inRunE = false
  leftPanel: HTMLElement | null
  rightPanel: HTMLElement | null
  initialFocusConsumed = false
  runWithCustomCallbacks<A>(f: () => A, eCallbacks: ECallbacks) {
    assert(!this.inRunE)
    this.inRunE = true
    try {
      let a = withEnvironment(new Environment(libraries, guidMap, guidRootViews, tryFirst(renders, defaultRender), clearGraphHighlightCallbacks(eCallbacks)), f)
      this.forceUpdate() // TODO
      return a
    } finally {
      this.inRunE = false }}
  runE<A>(f: () => A) {
    let {undoRedoArray, eCallbacks} = undoRedoECallbacks()
    let a = this.runWithCustomCallbacks(f, eCallbacks)
    if (undoRedoArray.length > 0) {
      undoStack.push(undoRedoArray)
      redoStack = [] }
    return a }
  activeEditorSupportsUnderselection(): boolean {
    return editorCommandsForActiveElement()?.newEdge !== undefined }
  updateMenuState() {
    progred.setMenuItemEnabled("new-edge", this.activeEditorSupportsUnderselection())
    progred.setMenuItemEnabled("delete", editorCommandsForActiveElement()?.commit !== undefined || graphHighlight !== nothing)
    progred.setMenuItemChecked("show-graph", this.showGraph) }
  setGraphSelection(nextGraphSelection: Maybe<GraphSelection>) {
    graphHighlight = nextGraphSelection
    this.forceUpdate() }
  focusSelection() {
    for (let root of [this.leftPanel, this.rightPanel])
      if (root && focusPendingEditor(root)) return
    if (this.initialFocusConsumed) return
    mapMaybe(initialFocusCursor, cursor => {
      for (let root of [this.leftPanel, this.rightPanel])
        if (root && focusEditorForCursor(root, cursor)) {
          this.initialFocusConsumed = true
          return {} } }) }
  render() {
    let documentRender = withEnvironment(new Environment(libraries, guidMap, guidRootViews, defaultRender, readOnlyECallbacks().eCallbacks), () =>
      bindMaybe(bindMaybe(environment().rootViews.root, ({id}) => Module.fromID(id)), renderFromModule) )
    let {rootDescend, viewsDescend} = withEnvironment(new Environment(libraries, guidMap, guidRootViews, tryFirst(dispatch(renders, libraryRender, ...maybeToArray(documentRender)), defaultRender), readOnlyECallbacks().eCallbacks), createD)
    let graphSnapshot = this.showGraph
      ? withEnvironment(new Environment(libraries, guidMap, guidRootViews, defaultRender, readOnlyECallbacks().eCallbacks), () =>
        buildGraphViewSnapshot(guidMap, guidRootViews, activeEdge(), graphHighlight))
      : nothing
    this.rootDescend = rootDescend
    this.viewsDescend = viewsDescend
    let hasSidebar = this.showGraph || viewsDescend !== nothing
    return <div style={{position: "absolute", top: 0, left: 0, right: 0, bottom: 0}}>
      <div ref={leftPanel => { this.leftPanel = leftPanel }} className={hasSidebar ? "leftPanel" : ""}
        style={{display: "inline-block", width: hasSidebar ? "60%" : "100%", height: "100%", overflow: "auto"}}
        onScroll={() => { if (this.rootDComponent) this.rootDComponent.onScroll() }} >
        <div className="doc"><DComponent
          ref={dComponent => { this.rootDComponent = dComponent }}
          d={this.rootDescend}
          depth={0}
          scrollParent={() => this.leftPanel}
          runE={f => this.runE(f)} /></div></div>
      {hasSidebar
        ? <div className="sidebar" style={{width: "40%", height: "100%", display: "inline-block"}}>
          <div className="separator" style={{height: "100%", display: "inline-block"}} />
          <div className="rightPanel" style={{width: "100%", height: "100%", display: "inline-block"}}>
            {maybe(graphSnapshot, () => null, graphSnapshot =>
              <div className="graphPanel" style={{height: viewsDescend === nothing ? "100%" : "50%"}}>
                <GraphViewComponent
                  snapshot={graphSnapshot}
                  setGraphSelection={selection => this.setGraphSelection(selection)}
                  chooseID={id => this.runE(() => commitIDToActiveElement(id))} />
              </div>)}
            {maybe(this.viewsDescend, () => null, viewsDescend =>
              <div ref={rightPanel => { this.rightPanel = rightPanel }} className="viewsPanel" style={{height: this.showGraph ? "50%" : "100%", overflow: "auto"}}
                onScroll={() => {if (this.viewsDComponent) this.viewsDComponent.onScroll()}} >
                <div className="views"><DComponent
                  ref={dComponent => { this.viewsDComponent = dComponent }}
                  d={viewsDescend}
                  depth={0}
                  scrollParent={() => this.rightPanel}
                  runE={f => this.runE(f)} /></div></div>)}
          </div></div>
        : null}</div> }
  onScroll() { if(this.rootDComponent) this.rootDComponent.onScroll(); if (this.viewsDComponent) this.viewsDComponent.onScroll() }
  componentDidMount() { this.onScroll(); this.focusSelection(); this.updateMenuState() }
  componentDidUpdate() { this.onScroll(); this.focusSelection(); this.updateMenuState() } }

window.onclick = () => { if (rootComponent) rootComponent.updateMenuState() }
window.addEventListener("focusin", () => { if (rootComponent) { rootComponent.forceUpdate(); rootComponent.updateMenuState() } })
window.addEventListener("focusout", () => { if (rootComponent) rootComponent.updateMenuState() })
window.onkeydown = e => { if (rootComponent) keyHandler(e, rootComponent.rootDescend, rootComponent.viewsDescend, f => rootComponent.runE(f)) }
progred.onMenuAction(action => { if (rootComponent) handleMenuAction(action) })

export let rootComponent: RootComponent
createRoot(document.getElementById('root') as HTMLElement)
  .render(<RootComponent ref={component => { if (component) rootComponent = component }} />)
