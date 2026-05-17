import * as React from "react"
import { withEnvironment } from "../Environment"
import { childContext, D, mergeEditorCommands, dElement, DScope, renderD, useDContext } from "./DContext"

export function collapsible(defaultCollapsed: boolean, singleLine: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D): D {
  return dElement(CollapsibleComponent, {defaultCollapsed, render}, {singleLine})
}

function CollapsibleComponent(props: {defaultCollapsed: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D}) {
  const [collapsed, setCollapsed] = React.useState(props.defaultCollapsed)
  const context = useDContext()
  let editorCommands = mergeEditorCommands(context.editorCommands, {collapse: () => setCollapsed(true)})
  return <DScope context={childContext(context, {editorCommands})}>{renderD(withEnvironment(context.environment, () => props.render(collapsed, setCollapsed)))}</DScope>
}

export function collapseToggle(collapsed: boolean, action: () => void): D {
  return dElement(CollapseToggleComponent, {collapsed, action}, {singleLine: true})
}

function CollapseToggleComponent(props: {collapsed: boolean, action: () => void}) {
  return <span className="collapseToggle" onClick={e => { e.stopPropagation(); props.action() }}>{props.collapsed ? "▸" : "▾"}</span>
}

export function button(text: string, action: () => void): D {
  return dElement(ButtonComponent, {text, action}, {singleLine: true})
}

function ButtonComponent(props: {text: string, action: () => void}) {
  const context = useDContext()
  return <input type="button" value={props.text} onClick={e => { e.stopPropagation(); context.runE(props.action) }} />
}
