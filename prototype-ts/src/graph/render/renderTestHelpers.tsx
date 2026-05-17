import * as React from "react"
import { act } from "react"
import { flushSync } from "react-dom"
import { createRoot } from "react-dom/client"
import { withEnvironment, type Environment } from "../Environment"
import { DRoot, type D } from "./D"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

export function renderDForTest(environment: Environment, d: D) {
  const container = document.createElement("div")
  document.body.appendChild(container)
  const root = createRoot(container)
  const runE = (f: () => void) => {
    withEnvironment(environment, f)
    render() }
  function render() {
    withEnvironment(environment, () => flushSync(() => root.render(
      <DRoot d={d} environment={environment} depth={0} runE={runE} />))) }

  act(render)
  return {
    container,
    unmount: () => act(() => {
      root.unmount()
      container.remove() }) }
}
