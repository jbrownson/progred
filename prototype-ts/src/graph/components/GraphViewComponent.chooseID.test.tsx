import * as React from "react"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { ID } from "../model/ID"
import type { GraphViewSnapshot } from "../graphView/GraphViewSnapshot"
import { GraphViewComponent } from "./GraphViewComponent"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

beforeEach(() => {
  vi.stubGlobal("requestAnimationFrame", () => 0)
  vi.stubGlobal("cancelAnimationFrame", () => {})
})

afterEach(() => {
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

function snapshot(): GraphViewSnapshot {
  return {
    nodes: [
      {id: "guid-source", label: {parts: [{name: undefined, guid: "guid-source"}]}, root: true},
      {id: "guid-target", label: {parts: [{name: undefined, guid: "guid-target"}]}, root: false}],
    edges: [
      {source: "guid-source", label: "guid-label", target: "guid-target", labelText: {parts: [{name: undefined, guid: "guid-label"}]}}],
    selectedNode: undefined,
    selectedEdge: undefined }
}

function clickAlt(element: Element) {
  element.dispatchEvent(new MouseEvent("mousedown", {bubbles: true, cancelable: true, altKey: true}))
  element.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true, altKey: true}))
}

describe("GraphViewComponent choose ID", () => {
  it("chooses graph nodes with alt-click", () => {
    let chosen: ID[] = []
    let container = document.createElement("div")
    document.body.appendChild(container)
    let root = createRoot(container)

    act(() => root.render(
      <GraphViewComponent
        snapshot={snapshot()}
        setGraphSelection={() => {}}
        chooseID={id => {
          chosen.push(id)
          return true }} />))

    let node = container.querySelector("g.graphNode") as SVGGElement
    expect(node).not.toBe(null)
    act(() => clickAlt(node))

    expect(chosen).toEqual(["guid-source"])

    act(() => root.unmount())
  })

  it("chooses graph edge labels with alt-click", () => {
    let chosen: ID[] = []
    let container = document.createElement("div")
    document.body.appendChild(container)
    let root = createRoot(container)

    act(() => root.render(
      <GraphViewComponent
        snapshot={snapshot()}
        setGraphSelection={() => {}}
        chooseID={id => {
          chosen.push(id)
          return true }} />))

    let edgeLabel = container.querySelector("g.graphEdgeLabel") as SVGGElement
    expect(edgeLabel).not.toBe(null)
    act(() => clickAlt(edgeLabel))

    expect(chosen).toEqual(["guid-label"])

    act(() => root.unmount())
  })
})
