import * as React from "react"
import { Environment, environment, withEnvironment } from "../Environment"
import { childContext, D, mergeEditorCommands, ProjectionContext, projectionElement, ProjectionScope } from "./ProjectionContext"

export function collapsible(defaultCollapsed: boolean, singleLine: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D): D {
  return projectionElement(CollapsibleComponent, {defaultCollapsed, render, environment: environment()}, "collapsible", singleLine)
}

function CollapsibleComponent(props: {defaultCollapsed: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D, environment: Environment}) {
  const [collapsed, setCollapsed] = React.useState(props.defaultCollapsed)
  const context = React.useContext(ProjectionContext)
  let editorCommands = mergeEditorCommands(context.editorCommands, {collapse: () => setCollapsed(true)})
  return <ProjectionScope context={childContext(context, {editorCommands})}>{withEnvironment(props.environment, () => props.render(collapsed, setCollapsed))}</ProjectionScope>
}

export function collapseToggle(collapsed: boolean, action: () => void): D {
  return projectionElement(CollapseToggleComponent, {collapsed, action}, "collapseToggle", true)
}

function CollapseToggleComponent(props: {collapsed: boolean, action: () => void}) {
  return <span className="collapseToggle" onClick={e => { e.stopPropagation(); props.action() }}>{props.collapsed ? "▸" : "▾"}</span>
}

export function button(text: string, action: () => void): D {
  return projectionElement(ButtonComponent, {text, action}, "button", true)
}

function ButtonComponent(props: {text: string, action: () => void}) {
  const context = React.useContext(ProjectionContext)
  return <input type="button" value={props.text} onClick={e => { e.stopPropagation(); context.runE(props.action) }} />
}
