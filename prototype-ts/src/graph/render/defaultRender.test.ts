import { afterEach, describe, expect, it } from "vitest"
import { act } from "react"
import { SourceType } from "../Environment"
import { appCtor, ctorField, nameField } from "../graph"
import type { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import { MapIDMap } from "../model/MapIDMap"
import { withTestEnvironment } from "../testHelpers"
import { dText } from "./D"
import { defaultRender, renderString } from "./defaultRender"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"
import { renderDForTest } from "./renderTestHelpers"
import { editorCommandsForActiveElement } from "../editor/EditorCommands"
import { renderIfApp } from "../renderIfs"

afterEach(() => {
  document.body.replaceChildren()
})

function edge(): Edge {
  return {parent: "guid-holder", label: sidFromString("root")}
}

describe("defaultRender", () => {
  it("renders missing edges as placeholders", () => {
    withTestEnvironment(environment => {
      const e = edge()

      const d = defaultRender(e, undefined)
      const {container, unmount} = renderDForTest(environment, d)

      expect(container.querySelector(".uneditable")?.textContent).toBe("[unnamed]")
      unmount()
    })
  })

  it("uses the edge context field name for placeholder labels", () => {
    withTestEnvironment(environment => {
      const d = defaultRender(edge(), undefined, {fieldName: "root"})
      const {container, unmount} = renderDForTest(environment, d)

      expect(container.querySelector(".uneditable")?.textContent).toBe("root")
      unmount()
    })
  })

  it("renders writable document GUIDs as focusable editors with new-edge support", () => {
    withTestEnvironment(environment => {
      const e = edge()
      const d = renderDocumentGuidEditor(e, {id: "guid-node", source: {source: SourceType.DocumentType, guid: "guid-node"}}, dText("node"))
      const {container, unmount} = renderDForTest(environment, d)

      const editor = container.querySelector(".guidEditor")
      expect(editor?.textContent).toBe("node")
      ;(editor as HTMLElement).focus()
      act(() => editorCommandsForActiveElement()?.newEdge?.())
      expect(container.querySelector("input.edgefield")?.getAttribute("placeholder")).toBe("label")
      unmount()
    })
  })

  it("renders library GUIDs as selectable editors without document edit commands", () => {
    withTestEnvironment(environment => {
      const e = edge()
      const d = renderDocumentGuidEditor(e, {id: "guid-node", source: {source: SourceType.LibraryType}}, dText("node"))
      const {container, unmount} = renderDForTest(environment, d)

      const editor = container.querySelector(".guidEditor") as HTMLElement
      expect(editor.textContent).toBe("node")
      editor.focus()
      expect(editorCommandsForActiveElement()?.copy?.().referenceID).toBe("guid-node")
      expect(editorCommandsForActiveElement()?.newEdge).toBe(undefined)
      unmount()
    })
  })

  it("renders library GUIDs reached through document edges without document edit commands", () => {
    const libraryMap = new Map([["guid-node", new Map([[nameField.id, sidFromString("Library Node")]])]])
    const libraries = new Map([["library", {idMap: new MapIDMap(libraryMap), root: "guid-node"}]])
    withTestEnvironment(environment => {
      const e = edge()
      const d = renderDocumentGuidEditor(e, {id: "guid-node", source: {source: SourceType.DocumentType, guid: "guid-holder"}}, dText("node"))
      const {container, unmount} = renderDForTest(environment, d)

      const editor = container.querySelector(".guidEditor") as HTMLElement
      editor.focus()
      expect(editorCommandsForActiveElement()?.copy?.().referenceID).toBe("guid-node")
      expect(editorCommandsForActiveElement()?.newEdge).toBe(undefined)
      unmount()
    }, {libraries})
  })

  it("renders generated render children under library GUIDs without document edit commands", () => {
    const libraryMap = new Map([["guid-app", new Map([[ctorField.id, appCtor.id], [nameField.id, sidFromString("Library App")]])]])
    const libraries = new Map([["library", {idMap: new MapIDMap(libraryMap), root: "guid-app"}]])
    withTestEnvironment(environment => {
      const d = renderIfApp(name => name)(edge(), {id: "guid-app", source: {source: SourceType.DocumentType, guid: "guid-holder"}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)

      ;(container.querySelector("textarea.string") as HTMLElement).focus()
      expect(editorCommandsForActiveElement()?.commit).toBe(undefined)
      unmount()
    }, {libraries, defaultRender})
  })

  it("renders named labels as text in fields", () => {
    withTestEnvironment(environment => {
      const label = "guid-label"
      environment.guidMap.set(label, nameField.id, sidFromString("Label"))

      const d = renderField("guid-node", label)
      const {container, unmount} = renderDForTest(environment, d)

      expect(container.querySelector(".edgeLabel")?.textContent).toBe("Label →")
      unmount()
    })
  })

  it("renders unnamed GUID labels as identicons in fields", () => {
    withTestEnvironment(environment => {
      const label = "guid-label"
      const d = renderField("guid-node", label)
      const {container, unmount} = renderDForTest(environment, d)

      expect(container.querySelector(".edgeLabel .identicon")).not.toBe(null)
      unmount()
    })
  })

  it("renders strings as string editors", () => {
    withTestEnvironment(environment => {
      const d = renderString(edge(), sidFromString("hello"), "hello", {source: SourceType.DocumentType, guid: "guid-holder"})
      const {container, unmount} = renderDForTest(environment, d)

      expect((container.querySelector("textarea.string") as HTMLTextAreaElement)?.value).toBe("hello")
      unmount()
    })
  })

  it("defaults indirect cycles collapsed when the same node is reached through different labels", () => {
    withTestEnvironment(environment => {
      const a = "guid-a"
      const b = "guid-b"
      environment.guidMap.set(a, sidFromString("left"), b)
      environment.guidMap.set(b, sidFromString("right"), a)
      const d = defaultRender(edge(), {id: a, source: {source: SourceType.DocumentType, guid: a}})
      const {container, unmount} = renderDForTest(environment, d)

      expect(Array.from(container.querySelectorAll(".collapseToggle")).filter(toggle => toggle.textContent === "▸")).toHaveLength(1)
      unmount()
    }, {defaultRender})
  })

  it("does not treat sibling references to the same node as cycles", () => {
    withTestEnvironment(environment => {
      const root = "guid-root"
      const shared = "guid-shared"
      environment.guidMap.set(root, sidFromString("left"), shared)
      environment.guidMap.set(root, sidFromString("right"), shared)
      environment.guidMap.set(shared, nameField.id, sidFromString("Shared"))
      const d = defaultRender(edge(), {id: root, source: {source: SourceType.DocumentType, guid: root}})
      const {container, unmount} = renderDForTest(environment, d)

      expect(Array.from(container.querySelectorAll(".collapseToggle")).filter(toggle => toggle.textContent === "▸")).toHaveLength(0)
      unmount()
    }, {defaultRender})
  })
})
