import { describe, expect, it } from "vitest"
import { aField, addCtor, bField, constantCtor, constantField, ctorField, emptyListCtor, externFunctionCtor, headField, nameField, nonemptyListCtor, parameterCtor, parametersField, tailField } from "../graph"
import type { SerializedGraph } from "../model/save"
import { findGraph, inspectGraph, loadSerializedGraph, prettyStaticMarkup, renderGraph, runGraphCLI } from "./graphCLI"

function graph(): SerializedGraph {
  return {
    root: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    guidMap: {
      aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: [
        {label: {guid: ctorField.id}, to: {guid: externFunctionCtor.id}},
        {label: {guid: nameField.id}, to: {string: "add"}},
        {label: {guid: parametersField.id}, to: {guid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}},
      ],
      bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: [
        {label: {guid: ctorField.id}, to: {guid: nonemptyListCtor.id}},
        {label: {guid: headField.id}, to: {guid: "cccccccccccccccccccccccccccccccc"}},
        {label: {guid: tailField.id}, to: {guid: "dddddddddddddddddddddddddddddddd"}},
      ],
      cccccccccccccccccccccccccccccccc: [
        {label: {guid: ctorField.id}, to: {guid: parameterCtor.id}},
        {label: {guid: nameField.id}, to: {string: "x"}},
      ],
      dddddddddddddddddddddddddddddddd: [
        {label: {guid: ctorField.id}, to: {guid: nonemptyListCtor.id}},
        {label: {guid: headField.id}, to: {guid: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}},
        {label: {guid: tailField.id}, to: {guid: "ffffffffffffffffffffffffffffffff"}},
      ],
      eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee: [
        {label: {guid: ctorField.id}, to: {guid: parameterCtor.id}},
        {label: {guid: nameField.id}, to: {string: "y"}},
      ],
      ffffffffffffffffffffffffffffffff: [
        {label: {guid: ctorField.id}, to: {guid: emptyListCtor.id}},
      ],
    },
  }
}

describe("graph CLI", () => {
  it("finds nodes by names supplied by the graph and libraries", () => {
    const loadedGraph = loadSerializedGraph(graph())

    expect(findGraph(loadedGraph, "add")).toContain("document aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa (add)")
    expect(findGraph(loadedGraph, "Extern Function")).toContain("ctor: Extern Function")
  })

  it("inspects nodes and expands list-valued edges", () => {
    const loadedGraph = loadSerializedGraph(graph())
    const output = inspectGraph(loadedGraph, "add")

    expect(output).toContain("id: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa (add)")
    expect(output).toContain("ctor: d4712396ae66b10862773f9f90245f68 (Extern Function)")
    expect(output).toContain("parameters")
    expect(output).toContain("0: cccccccccccccccccccccccccccccccc (x)")
    expect(output).toContain("1: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee (y)")
  })

  it("renders graph projections as static markup", () => {
    const loadedGraph = loadSerializedGraph(graph())
    const output = renderGraph(loadedGraph, "add")

    expect(output).toContain("extern ")
    expect(output).toContain(">add</textarea>")
    expect(output).toContain(">x</span>")
    expect(output).toContain(">y</span>")
    expect(output).toContain("guidEditor")
    expect(output).toContain("\n  <span")
  })

  it("renders Scene3D implicit expression templates as static markup", () => {
    const loadedGraph = loadSerializedGraph({
      root: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      guidMap: {
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: [
          {label: {guid: ctorField.id}, to: {guid: addCtor.id}},
          {label: {guid: aField.id}, to: {guid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}},
          {label: {guid: bField.id}, to: {guid: "cccccccccccccccccccccccccccccccc"}},
        ],
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: [
          {label: {guid: ctorField.id}, to: {guid: constantCtor.id}},
          {label: {guid: constantField.id}, to: {number: 1}},
        ],
        cccccccccccccccccccccccccccccccc: [
          {label: {guid: ctorField.id}, to: {guid: constantCtor.id}},
          {label: {guid: constantField.id}, to: {number: 2}},
        ],
      },
    })
    const output = renderGraph(loadedGraph, "root")

    expect(output).toContain("<span>(</span>")
    expect(output).toContain("<span> + </span>")
    expect(output).toContain("value=\"1\"")
    expect(output).toContain("value=\"2\"")
  })

  it("reports usage for incomplete commands", () => {
    const result = runGraphCLI(["find", "graph.progred"])

    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("Usage")
  })

  it("pretty prints static markup", () => {
    expect(prettyStaticMarkup("<span><span>a</span><br/><span> <span>b</span></span></span>")).toBe([
      "<span>",
      "  <span>a</span>",
      "  <br/>",
      "  <span> ",
      "    <span>b</span>",
      "  </span>",
      "</span>",
    ].join("\n"))
  })
})
