import * as React from "react"
import { concatMap, intersperse, join } from "../../lib/Array"
import { maybe, Maybe, nothing } from "../../lib/Maybe"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { commitToActiveElementWithRefocus } from "../editor/commitWithFocus"
import { EditorCommands } from "../editor/EditorCommands"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { focusEditorFromElement } from "../editor/EditorFocus"
import { IdenticonComponent } from "../components/IdenticonComponent"
import { ListInsertionEditorComponent } from "../components/ListInsertionEditorComponent"
import { GUID } from "../model/ID"
import { childContext, D, DContextValue, isBlock, isSingleLine, mergeEditorCommands, dElement, DScope, renderD, useDContext } from "./DContext"

const indentWidth = 16

export function block(...children: D[]): D {
  return dElement(BlockComponent, {children}, {singleLine: false, block: true})
}

function BlockComponent(props: {children: D[]}) {
  const context = useDContext()
  return <span>{concatMap(props.children, (d, index) => isBlock(d)
    ? [<DScope key={`block${index}`} context={childContext(context, {depth: context.depth + 1})}>{renderD(d)}</DScope>]
    : [
      <br key={`br${index}`} />,
      <span key={`indent${index}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
      <DScope key={`d${index}`} context={childContext(context, {depth: context.depth + 1})}>{renderD(d)}</DScope>])}</span>
}

export function line(...children: D[]): D {
  return dElement(LineComponent, {children}, {singleLine: !children.find(child => !isSingleLine(child))})
}

function LineComponent(props: {children: D[]}) {
  return <span>{props.children.map((d, index) => <React.Fragment key={index}>{renderD(d)}</React.Fragment>)}</span>
}

export function dText(string: string): D {
  return dElement(TextComponent, {string}, {singleLine: true})
}

function TextComponent(props: {string: string}) {
  const context = useDContext()
  return <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.string}</span>
}

export function dIdenticon(guid: GUID, size = 16): D {
  return dElement(IdenticonDComponent, {guid, size}, {singleLine: true})
}

function IdenticonDComponent(props: {guid: GUID, size: number}) {
  const context = useDContext()
  return <span className="identicon" onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}><IdenticonComponent guid={props.guid} size={props.size} /></span>
}

function keepFocusForChooseID(e: React.MouseEvent) {
  if (chooseIDModifier(e)) {
    e.stopPropagation()
    e.preventDefault() }}

function selectOrChooseID(e: React.MouseEvent, context: DContextValue) {
  e.stopPropagation()
  if (chooseIDModifier(e)) {
    e.preventDefault()
    context.runE(() => maybe(context.chooseID?.(), () => false, commitToActiveElementWithRefocus))
    return }
  focusEditorFromElement(e.currentTarget) }

export type ListInsertionPoint = {
  entries: (needle: string) => {a: Entry, matches: Match[]}[]
  editorCommands: EditorCommands
  requiresMeta?: boolean }

export function dList(opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D> = nothing, insertionPoints: ListInsertionPoint[] = [], collapsed = false): D {
  return dElement(ListComponent, {opening, children, closing, separator, collapseToggle, insertionPoints, collapsed}, {singleLine: children.length <= 1 && !children.find(child => !isSingleLine(child))})
}

function ListComponent(props: {opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D>, insertionPoints: ListInsertionPoint[], collapsed: boolean}) {
  const context = useDContext()
  const [activeListInsertion, setActiveListInsertionState] = React.useState<number | undefined>(undefined)
  const activeInsertion = activeListInsertion !== undefined && props.insertionPoints[activeListInsertion] ? activeListInsertion : undefined
  const setActiveListInsertion = (i: number, active: boolean) => setActiveListInsertionState(activeListInsertion => active ? i : activeListInsertion === i ? undefined : activeListInsertion)
  let opening = <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.opening}</span>
  let closing = <span>{props.closing}</span>
  let insertionPoint = (i: number, label: string) => props.insertionPoints[i]
    ? <ListInsertionEditorComponent key={`insertion${i}`} insertionPoint={props.insertionPoints[i]} label={label} active={activeInsertion === i} setActive={active => setActiveListInsertion(i, active)} insertionIndex={i} descend={context.descend} runE={context.runE} />
    : <span key={`insertion${i}`}>{label}</span>
  let child = (d: D, i: number, depth: number) => {
    let insertionIndex = i + 1
    let editorCommands = props.insertionPoints[insertionIndex]
      ? mergeEditorCommands(context.editorCommands, {keyDown: e => e.key === "," && (e.metaKey || !props.insertionPoints[insertionIndex].requiresMeta) ? () => {
          e.preventDefault()
          e.stopPropagation()
          setActiveListInsertion(insertionIndex, true) } : nothing})
      : context.editorCommands
    return <DScope key={`child${i}`} context={childContext(context, {depth, editorCommands})}>{renderD(d)}</DScope> }
  let active = activeInsertion !== undefined
  let itemCount = props.children.length + (active ? 1 : 0)
  let singleLine = itemCount <= 1 && !props.children.find(child => !isSingleLine(child))
  let listItems = (depth: number) => {
    let items: React.ReactNode[] = []
    for (let i = 0; i <= props.children.length; i++) {
      if (activeInsertion === i) items.push(insertionPoint(i, ""))
      if (i < props.children.length) items.push(child(props.children[i], i, depth)) }
    return items }
  let inlineItems = (items: React.ReactNode[]) => [<span key="leading"> </span>, ...join(intersperse(items.map(item => [item]), i => [<span key={`separator${i}`}>{props.separator} </span>])), <span key="trailing"> </span>]
  let multilineItems = (items: React.ReactNode[]) => join(items.map((item, i, items) => [
    <br key={`br${i}`} />,
    <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
    item,
    i + 1 < items.length ? <span key={`separator${i}`}>{props.separator}</span> : null]))
  let inactiveInline = () => [insertionPoint(0, " "), ...concatMap(props.children, (d, i) => [
    child(d, i, context.depth),
    insertionPoint(i + 1, " ")])]
  let inactiveMultiline = () => [insertionPoint(0, " "), ...join(intersperse(
    props.children.map((d, i) => [
      <br key={`br${i}`} />,
      <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
      child(d, i, context.depth + 1),
      i === props.children.length - 1 ? insertionPoint(props.children.length, " ") : null]),
    i => [insertionPoint(i, props.separator)]))]
  let content = props.collapsed
    ? [<span key="collapsed" className="collapsedListContents">...</span>]
    : active
    ? singleLine ? inlineItems(listItems(context.depth)) : multilineItems(listItems(context.depth + 1))
    : singleLine
    ? inactiveInline()
    : inactiveMultiline()
  return <span>{maybe(props.collapseToggle, () => null, renderD)}{opening}{content}{closing}</span>
}

export { indentWidth }
