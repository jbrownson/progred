import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import * as React from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { bindMaybe, fromMaybe, mapMaybe, maybe, Maybe, nothing } from "../../lib/Maybe"
import { noopECallbacks } from "../editor/ECallbacks"
import { edges, Environment, SourceType, withEnvironment } from "../Environment"
import { arrayFromList } from "../list"
import { ctorField, listFromID, Module, nameField } from "../graph"
import { libraries } from "../libraries/libraries"
import { GUIDMap } from "../model/GUIDMap"
import { ID, matchID, nidFromNumber, sidFromString, stringFromID } from "../model/ID"
import { load } from "../model/load"
import type { SerializedGraph } from "../model/save"
import { defaultRender, tryFirst } from "../render/defaultRender"
import { DRoot } from "../render/DRoot"
import { createProjection } from "../render/project"
import { dispatch, Render } from "../render/R"
import { renderFromLibraries, renderFromModule } from "../render/renderFromLibraries"
import { renders } from "../render/renders"

export type LoadedGraph = {
  file: string
  root: Maybe<ID>
  guidMap: GUIDMap
  environment: Environment
}

type NodeSource = {
  source: string
  id: ID
  edges: Map<ID, ID>
}

export function loadGraphFile(file: string): LoadedGraph {
  return loadSerializedGraph(JSON.parse(readFileSync(file, "utf8")) as SerializedGraph, file)
}

export function loadSerializedGraph(serializedGraph: SerializedGraph, file = "<graph>"): LoadedGraph {
  const {root, guidMap} = load(serializedGraph)
  const environment = new Environment(
    libraries,
    guidMap,
    {id: "graph-cli-workspace", root, view: nothing},
    () => { throw new Error("The graph CLI does not have a default render") },
    noopECallbacks)
  return {file, root, guidMap, environment}
}

export function findGraph(loadedGraph: LoadedGraph, query: string): string {
  const q = query.toLowerCase()
  return withEnvironment(loadedGraph.environment, () => {
    const matches = allNodes(loadedGraph)
      .map(node => ({...node, reasons: matchReasons(node.id, node.edges, q)}))
      .filter(node => node.reasons.length > 0)
    return matches.length === 0
      ? `No matches for ${JSON.stringify(query)}`
      : matches.map(node => `${node.source} ${idSummary(node.id)}\n  ${node.reasons.join("\n  ")}`).join("\n") }) }

export function inspectGraph(loadedGraph: LoadedGraph, target: Maybe<string>): string {
  return withEnvironment(loadedGraph.environment, () => {
    const id = resolveTarget(loadedGraph, target)
    if (id === nothing) return `Could not resolve ${JSON.stringify(target)}`
    const nodeEdges = edges(id)
    const ctor = bindMaybe(nodeEdges, ({edges}) => edges.get(ctorField.id))
    const lines = [
      `file: ${loadedGraph.file}`,
      `root: ${maybe(loadedGraph.root, () => "[none]", idSummary)}`,
      `id: ${idSummary(id)}`,
      `source: ${maybe(nodeEdges, () => "none", ({source}) => source.source === SourceType.DocumentType ? "document" : "library")}`,
      `ctor: ${maybe(ctor, () => "[none]", idSummary)}` ]
    mapMaybe(listItems(id), items => {
      lines.push(`list: ${items.length} item${items.length === 1 ? "" : "s"}`)
      items.forEach((item, i) => lines.push(`  ${i}: ${idSummary(item)}`)) })
    if (nodeEdges === undefined) return lines.join("\n")
    const renderedEdges = Array.from(nodeEdges.edges.entries())
      .sort(([a], [b]) => idSortKey(a).localeCompare(idSortKey(b)))
    if (renderedEdges.length > 0) lines.push("edges:")
    renderedEdges.forEach(([label, to]) => {
      lines.push(`  ${idSummary(label)} -> ${idSummary(to)}`)
      mapMaybe(listItems(to), items => items.forEach((item, i) => lines.push(`    ${i}: ${idSummary(item)}`))) })
    return lines.join("\n") }) }

export function renderGraph(loadedGraph: LoadedGraph, target: Maybe<string>): string {
  return withEnvironment(loadedGraph.environment, () => {
    const id = resolveTarget(loadedGraph, target)
    if (id === nothing) return `Could not resolve ${JSON.stringify(target)}`
    const previousRoot = loadedGraph.environment.workspace.root
    const previousDefaultRender = loadedGraph.environment.defaultRender
    const render = dispatch(...graphRenders(loadedGraph))
    loadedGraph.environment.workspace.root = id
    loadedGraph.environment.defaultRender = tryFirst(render, defaultRender)
    try {
      const d = createProjection(render).rootDescend
      return prettyStaticMarkup(renderToStaticMarkup(React.createElement(DRoot, {
        d,
        environment: loadedGraph.environment,
        depth: 0,
        runE: (f: () => void) => withEnvironment(loadedGraph.environment, f) }))) }
    finally {
      loadedGraph.environment.workspace.root = previousRoot
      loadedGraph.environment.defaultRender = previousDefaultRender } }) }

export function runGraphCLI(argv: string[]): {exitCode: number, stdout: string, stderr: string} {
  const [command, file, ...rest] = argv
  if (command === undefined || file === undefined || command === "help")
    return {exitCode: command === "help" || command === undefined ? 0 : 1, stdout: usage(), stderr: ""}
  if (command === "find" && rest.length === 0) return {exitCode: 1, stdout: "", stderr: usage()}
  if (command !== "find" && command !== "inspect" && command !== "render")
    return {exitCode: 1, stdout: "", stderr: `Unknown command ${JSON.stringify(command)}\n${usage()}`}
  try {
    const loadedGraph = loadGraphFile(resolve(file))
    if (command === "find") return {exitCode: 0, stdout: findGraph(loadedGraph, rest.join(" ")), stderr: ""}
    if (command === "inspect") return {exitCode: 0, stdout: inspectGraph(loadedGraph, rest[0]), stderr: ""}
    if (command === "render") return {exitCode: 0, stdout: renderGraph(loadedGraph, rest[0]), stderr: ""}
    return {exitCode: 1, stdout: "", stderr: usage()}
  } catch (e) {
    return {exitCode: 1, stdout: "", stderr: e instanceof Error ? e.message : `${e}`} } }

function usage() {
  return [
    "Usage:",
    "  npm run graph -- find <file.progred> <text>",
    "  npm run graph -- inspect <file.progred> [root|id|name]",
    "  npm run graph -- render <file.progred> [root|id|name]",
  ].join("\n") }

export function prettyStaticMarkup(markup: string): string {
  const tokens = markup.match(/<[^>]*>|[^<]+/g) || []
  const lines: string[] = []
  let depth = 0
  for (let i = 0; i < tokens.length;) {
    const token = tokens[i]
    if (token.startsWith("</")) {
      depth--
      lines.push(`${indent(depth)}${token}`)
      i++
    } else if (token.startsWith("<")) {
      const tag = tagName(token)
      const next = tokens[i + 1]
      const nextNext = tokens[i + 2]
      if (tag !== undefined && next === `</${tag}>`) {
        lines.push(`${indent(depth)}${token}${next}`)
        i += 2
      } else if (tag !== undefined && next !== undefined && !next.startsWith("<") && nextNext === `</${tag}>`) {
        lines.push(`${indent(depth)}${token}${next}${nextNext}`)
        i += 3
      } else if (next !== undefined && !next.startsWith("<") && next.trim() === "") {
        lines.push(`${indent(depth)}${token}${next}`)
        if (!selfClosingTag(token)) depth++
        i += 2
      } else {
        lines.push(`${indent(depth)}${token}`)
        if (!selfClosingTag(token)) depth++
        i++ }
    } else {
      lines.push(`${indent(depth)}${token}`)
      i++ }}
  return lines.join("\n") }

function tagName(token: string): Maybe<string> {
  return token.match(/^<\/?([^\s>/]+)/)?.[1] }

function selfClosingTag(token: string): boolean {
  return token.endsWith("/>") || ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"].includes(fromMaybe(tagName(token), () => "")) }

function indent(depth: number) {
  return "  ".repeat(Math.max(depth, 0)) }

function graphRenders(loadedGraph: LoadedGraph): Render[] {
  const moduleRenders = (id: ID) => maybe(bindMaybe(Module.fromID(id), renderFromModule), () => [], render => [render])
  return [
    renders,
    renderFromLibraries(libraries),
    ...maybe(loadedGraph.root, () => [], moduleRenders) ] }

function resolveTarget(loadedGraph: LoadedGraph, target: Maybe<string>): Maybe<ID> {
  if (target === undefined || target === "root") return loadedGraph.root
  const parsed = parseID(target)
  if (parsed !== nothing && hasKnownID(loadedGraph, parsed)) return parsed
  const exactMatches = allNodes(loadedGraph)
    .filter(node => nodeName(node.id)?.toLowerCase() === target.toLowerCase())
  return exactMatches[0]?.id || parsed }

function parseID(target: string): Maybe<ID> {
  if (target.startsWith("sid:")) return target
  if (/^-?\d+(\.\d+)?$/.test(target)) return nidFromNumber(Number(target))
  if (/^\".*\"$/.test(target)) {
    try {
      const string = JSON.parse(target)
      return typeof string === "string" ? sidFromString(string) : nothing
    } catch (_) {
      return nothing }}
  return /^[0-9a-f]{32}$/.test(target) ? target : nothing }

function hasKnownID(loadedGraph: LoadedGraph, id: ID) {
  if (id === loadedGraph.root) return true
  return matchID(id,
    guid => loadedGraph.guidMap.map.get(guid) !== undefined || allNodes(loadedGraph).some(node => node.id === guid),
    () => true,
    () => true) }

function allNodes(loadedGraph: LoadedGraph): NodeSource[] {
  const seen = new Set<ID>()
  const addNodes = (source: string, map: Map<string, Map<ID, ID>>) => Array.from(map.entries()).flatMap(([id, edges]) => {
    if (seen.has(id)) return []
    seen.add(id)
    return [{source, id, edges}] })
  return [
    ...addNodes("document", loadedGraph.guidMap.map),
    ...Array.from(libraries.entries()).flatMap(([source, {idMap}]) =>
      "map" in idMap && idMap.map instanceof Map ? addNodes(source, idMap.map) : []) ] }

function matchReasons(id: ID, nodeEdges: Map<ID, ID>, query: string): string[] {
  const values = [
    ["id", idForDisplay(id)],
    ["name", nodeName(id)],
    ["ctor", bindMaybe(nodeEdges.get(ctorField.id), nodeName)] ]
  const edgeValues = Array.from(nodeEdges.entries()).flatMap(([label, to]) => [
    [`label ${idForDisplay(label)}`, nodeName(label) || stringFromID(label)],
    [`value ${idForDisplay(label)}`, nodeName(to) || stringFromID(to)] ] as [string, Maybe<string>][])
  return [...values, ...edgeValues].flatMap(([kind, value]) =>
    value !== undefined && value.toLowerCase().includes(query) ? [`${kind}: ${value}`] : []) }

function nodeName(id: ID): Maybe<string> {
  return bindMaybe(edges(id), ({edges}) => bindMaybe(edges.get(nameField.id), stringFromID)) }

function listItems(id: ID): Maybe<ID[]> {
  return bindMaybe(listFromID(id, id => ({id})), list => mapMaybe(arrayFromList(list), items => items.map(({id}) => id))) }

function idSummary(id: ID): string {
  return matchID(id,
    guid => `${guid}${nameSuffix(guid)}`,
    (_sid, string) => JSON.stringify(string),
    number => `${number}`) }

function nameSuffix(id: ID) {
  const name = nodeName(id)
  const ctor = bindMaybe(bindMaybe(edges(id), ({edges}) => edges.get(ctorField.id)), nodeName)
  return name !== undefined ? ` (${name})` : ctor !== undefined ? ` <${ctor}>` : "" }

function idForDisplay(id: ID): string {
  return matchID(id, guid => `${guid}${nameSuffix(guid)}`, (_sid, string) => string, number => `${number}`)
}

function idSortKey(id: ID) {
  return fromMaybe(nodeName(id), () => idForDisplay(id)) }
