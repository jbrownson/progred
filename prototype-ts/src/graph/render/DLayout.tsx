import * as React from "react"
import { concatMap, intersperse, join } from "../../lib/Array"
import { mapMaybe, maybe, Maybe, nothing } from "../../lib/Maybe"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { commitIDToActiveElement, EditorCommands } from "../editor/EditorCommands"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { focusEditorForCursor } from "../editor/EditorFocus"
import { IdenticonComponent } from "../components/IdenticonComponent"
import { ListInsertionEditorComponent } from "../components/ListInsertionEditorComponent"
import { GUID } from "../model/ID"
import { childContext, D, isBlock, isSingleLine, mergeEditorCommands, DContext, dElement, DScope } from "./DContext"

const indentWidth = 16

export function block(...children: D[]): D {
  return dElement(BlockComponent, {children}, "block", false)
}

function BlockComponent(props: {children: D[]}) {
  const context = React.useContext(DContext)
  return <span>{concatMap(props.children, (d, index) => isBlock(d)
    ? [<DScope key={`block${index}`} context={childContext(context, {depth: context.depth + 1})}>{d}</DScope>]
    : [
      <br key={`br${index}`} />,
      <span key={`indent${index}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
      <DScope key={`d${index}`} context={childContext(context, {depth: context.depth + 1})}>{d}</DScope>])}</span>
}

export function line(...children: D[]): D {
  return dElement(LineComponent, {children}, "line", !children.find(child => !isSingleLine(child)))
}

function LineComponent(props: {children: D[]}) {
  return <span>{props.children.map((d, index) => <React.Fragment key={index}>{d}</React.Fragment>)}</span>
}

export function dText(string: string): D {
  return dElement(TextComponent, {string}, "text", true)
}

function TextComponent(props: {string: string}) {
  const context = React.useContext(DContext)
  return <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.string}</span>
}

export function dIdenticon(guid: GUID, size = 16): D {
  return dElement(IdenticonDComponent, {guid, size}, "identicon", true)
}

function IdenticonDComponent(props: {guid: GUID, size: number}) {
  const context = React.useContext(DContext)
  return <span className="identicon" onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}><IdenticonComponent guid={props.guid} size={props.size} /></span>
}

function keepFocusForChooseID(e: React.MouseEvent) {
  if (chooseIDModifier(e)) {
    e.stopPropagation()
    e.preventDefault() }}

function selectOrChooseID(e: React.MouseEvent, context: React.ContextType<typeof DContext>) {
  e.stopPropagation()
  if (chooseIDModifier(e)) {
    e.preventDefault()
    context.runE(() => maybe(context.chooseID?.(), () => false, commitIDToActiveElement))
    return }
  mapMaybe(context.focusCursor, cursor => focusEditorForCursor(document.body, cursor)) }

export type ListInsertionPoint = {
  entries: (needle: string) => {a: Entry, matches: Match[]}[]
  editorCommands: EditorCommands
  requiresMeta?: boolean }

export function dList(opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D> = nothing, insertionPoints: ListInsertionPoint[] = []): D {
  return dElement(ListComponent, {opening, children, closing, separator, collapseToggle, insertionPoints}, "list", children.length <= 1 && !children.find(child => !isSingleLine(child)))
}

function ListComponent(props: {opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D>, insertionPoints: ListInsertionPoint[]}) {
  const context = React.useContext(DContext)
  const [activeListInsertion, setActiveListInsertionState] = React.useState<number | undefined>(undefined)
  const activeInsertion = activeListInsertion !== undefined && props.insertionPoints[activeListInsertion] ? activeListInsertion : undefined
  const setActiveListInsertion = (i: number, active: boolean) => setActiveListInsertionState(activeListInsertion => active ? i : activeListInsertion === i ? undefined : activeListInsertion)
  let opening = <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.opening}</span>
  let closing = <span>{props.closing}</span>
  let singleLine = props.children.length <= 1 && !props.children.find(child => !isSingleLine(child))
  let insertionPoint = (i: number, label: string) => props.insertionPoints[i]
    ? <ListInsertionEditorComponent key={`insertion${i}`} insertionIndex={i} insertionPoint={props.insertionPoints[i]} label={label} active={activeInsertion === i} setActive={active => setActiveListInsertion(i, active)} scrollParent={context.scrollParent} runE={context.runE} />
    : <span key={`insertion${i}`}>{label}</span>
  let child = (d: D, i: number, depth: number) => {
    let insertionIndex = i + 1
    let editorCommands = props.insertionPoints[insertionIndex]
      ? mergeEditorCommands(context.editorCommands, {keyDown: e => e.key === "," && (e.metaKey || !props.insertionPoints[insertionIndex].requiresMeta) ? () => {
          e.preventDefault()
          e.stopPropagation()
          setActiveListInsertion(insertionIndex, true) } : nothing})
      : context.editorCommands
    return <DScope key={`child${i}`} context={childContext(context, {depth, editorCommands})}>{d}</DScope> }
  let activeItems = (depth: number) => {
    let items: React.ReactNode[] = []
    for (let i = 0; i <= props.children.length; i++) {
      if (activeInsertion === i) items.push(insertionPoint(i, ""))
      if (i < props.children.length) items.push(child(props.children[i], i, depth)) }
    return items }
  let content = props.collapseToggle && (props.collapseToggle.props as unknown as {collapsed: boolean}).collapsed
    ? [<span key="collapsed" className="collapsedListContents">...</span>]
    : activeInsertion !== undefined && singleLine
    ? [<span key="leading"> </span>, ...join(intersperse(activeItems(context.depth).map(item => [item]), i => [<span key={`separator${i}`}>{props.separator} </span>])), <span key="trailing"> </span>]
    : activeInsertion !== undefined
    ? join(activeItems(context.depth + 1).map((item, i, items) => [
        <br key={`br${i}`} />,
        <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
        item,
        i + 1 < items.length ? <span key={`separator${i}`}>{props.separator}</span> : null]))
    : singleLine
    ? [insertionPoint(0, " "), ...concatMap(props.children, (d, i) => [
      child(d, i, context.depth),
      insertionPoint(i + 1, " ")])]
    : [insertionPoint(0, " "), ...join(intersperse(
      props.children.map((d, i) => [
        <br key={`br${i}`} />,
        <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
        child(d, i, context.depth + 1),
        i === props.children.length - 1 ? insertionPoint(props.children.length, " ") : null]),
      i => [insertionPoint(i, props.separator)]))]
  return <span>{props.collapseToggle}{opening}{content}{closing}</span>
}

export { indentWidth }
