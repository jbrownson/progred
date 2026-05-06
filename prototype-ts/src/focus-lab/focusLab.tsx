import * as React from "react"
import { createRoot } from "react-dom/client"
import "./focusLab.css"

function assertFocus(expected: string, label: string) {
  requestAnimationFrame(() => {
    let actual = (document.activeElement as HTMLElement).dataset.focusKey
    console.log(`${actual === expected ? "PASS" : "FAIL"} ${label}`, {expected, actual})
    console.assert(actual === expected, `${label}: expected ${expected}, actual ${actual}`) }) }

function App() {
  const [active, setActive] = React.useState(false)
  const inputRef = React.useRef<HTMLInputElement | null>(null)

  React.useLayoutEffect(() => {
    if (active) {
      inputRef.current?.focus()
      assertFocus("middle-input", "middle replacement receives focus") }}, [active])

  function targetProps(key: string) {
    return {
      "data-focus-key": key,
      tabIndex: 0,
      onFocus: () => {
        console.log("focus", key)
        assertFocus(key, `${key} keeps focus`) },
      onBlur: (e: React.FocusEvent<HTMLElement>) => console.log("blur", key, "->", (e.relatedTarget as HTMLElement | null)?.dataset.focusKey),
    } as const }

  return <main>
    <div className="row">
      <span className="target" {...targetProps("before")}>before</span>
      {active
        ? <input
            ref={inputRef}
            className="target inputTarget"
            data-focus-key="middle-input"
            defaultValue="middle input"
            onFocus={() => console.log("focus middle-input")}
            onBlur={e => {
              console.log("blur middle-input ->", (e.relatedTarget as HTMLElement | null)?.dataset.focusKey)
              setActive(false)
              requestAnimationFrame(() => console.log("after middle-input blur", {activeElement: (document.activeElement as HTMLElement).dataset.focusKey})) }} />
        : <span
            className="target"
            data-focus-key="middle"
            tabIndex={0}
            onFocus={() => {
              console.log("focus middle")
              setActive(true) }}>
            middle
          </span>}
      <span className="target" {...targetProps("after")}>after</span>
    </div>
  </main> }

createRoot(document.getElementById("root")!).render(<App />)
