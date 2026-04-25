import * as React from "react"
import { Maybe, maybe } from "../../lib/Maybe"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { GraphEdge, GraphLabel, GraphLabelPart, GraphNode, GraphSelection, GraphSelectionStrength, GraphViewSnapshot } from "../graphView/GraphViewSnapshot"
import { IdenticonComponent } from "./IdenticonComponent"
import { ID, matchID } from "../model/ID"

type Point = {x: number, y: number}

const repulsionK = 8000
const attractionK = 0.02
const restLength = 120
const damping = 0.85
const maxForce = 10
const gravityK = 0.005

const textFontSize = 10
const textPadding = 8
const nodeIconSize = 28
const labelIconSize = 16
const edgeLabelIconSize = 18
const edgeLabelPadding = 4
const labelPartGap = 4
const textFont = `500 ${textFontSize}px sans-serif`
let textMeasureContext: CanvasRenderingContext2D | null = null
let textWidthCache = new Map<string, number>()

function add(a: Point, b: Point): Point { return {x: a.x + b.x, y: a.y + b.y} }
function sub(a: Point, b: Point): Point { return {x: a.x - b.x, y: a.y - b.y} }
function scale(a: Point, n: number): Point { return {x: a.x * n, y: a.y * n} }
function length(a: Point): number { return Math.hypot(a.x, a.y) }
function lengthSq(a: Point): number { return a.x * a.x + a.y * a.y }
function normalize(a: Point): Point {
  const l = length(a)
  return l < 0.001 ? {x: 1, y: 0} : scale(a, 1 / l) }

function hashString(s: string) {
  let hash = 2166136261
  for (let i = 0; i < s.length; i++)
    hash = Math.imul(hash ^ s.charCodeAt(i), 16777619)
  return hash >>> 0 }

function idKey(id: ID): string {
  return matchID(id,
    guid => `g:${guid}`,
    sid => `s:${sid}`,
    nid => `n:${nid}`) }

function deterministicPosition(id: ID, index: number): Point {
  const hash = hashString(idKey(id))
  return {
    x: (((hash & 0xFFFF) / 65535) - 0.5) * 300 + index * 5,
    y: ((((hash >>> 16) & 0xFFFF) / 65535) - 0.5) * 200 + index * 5 } }

function textWidth(text: string) {
  return maybe(textWidthCache.get(text), () => {
    if (!textMeasureContext) textMeasureContext = document.createElement("canvas").getContext("2d")
    const width = textMeasureContext ? (textMeasureContext.font = textFont, textMeasureContext.measureText(text).width) : text.length * 6
    textWidthCache.set(text, width)
    return width }, width => width) }

function truncateLabel(text: string) {
  return text.length <= 18 ? text : text.slice(0, 17) + "..." }

function graphLabelPartWidth(part: GraphLabelPart, iconSize: number) {
  return maybe(part.name,
    () => part.guid === undefined ? 0 : iconSize,
    name => textWidth(truncateLabel(name))) }

function graphLabelSize(label: GraphLabel, iconSize: number): Point {
  return {
    x: label.parts.reduce((width, part) => width + graphLabelPartWidth(part, iconSize), Math.max(0, label.parts.length - 1) * labelPartGap),
    y: Math.max(textFontSize, iconSize) } }

function nodeSize(node: GraphNode): Point {
  const labelSize = graphLabelSize(node.label, labelIconSize)
  return {x: labelSize.x + textPadding, y: labelSize.y + textPadding} }

function edgeLabelSize(edge: GraphEdge): Point {
  const labelSize = graphLabelSize(edge.labelText, edgeLabelIconSize)
  return {x: labelSize.x + edgeLabelPadding, y: labelSize.y + edgeLabelPadding} }

function pointOnQuadratic(start: Point, control: Point, end: Point, t: number): Point {
  const mt = 1 - t
  return add(add(scale(start, mt * mt), scale(control, 2 * mt * t)), scale(end, t * t)) }

function pointOnCubic(start: Point, cp1: Point, cp2: Point, end: Point, t: number): Point {
  const mt = 1 - t
  return add(add(scale(start, mt * mt * mt), scale(cp1, 3 * mt * mt * t)), add(scale(cp2, 3 * mt * t * t), scale(end, t * t * t))) }

function clipToRect(center: Point, half: Point, target: Point): Point {
  const dir = sub(target, center)
  if (Math.abs(dir.x) < 0.001 && Math.abs(dir.y) < 0.001) return add(center, {x: half.x, y: 0})
  const sx = Math.abs(dir.x) > 0.001 ? half.x / Math.abs(dir.x) : Number.MAX_VALUE
  const sy = Math.abs(dir.y) > 0.001 ? half.y / Math.abs(dir.y) : Number.MAX_VALUE
  return add(center, scale(dir, Math.min(sx, sy))) }

function clipToRectToward(center: Point, half: Point, control: Point, fallback: Point): Point {
  return lengthSq(sub(control, center)) > 1 ? clipToRect(center, half, control) : clipToRect(center, half, fallback) }

function canonicalPair(a: ID, b: ID) {
  const ak = idKey(a)
  const bk = idKey(b)
  return ak <= bk ? `${ak}|${bk}` : `${bk}|${ak}` }

function graphSelectionClass(strength: Maybe<GraphSelectionStrength>) {
  return strength === "primary" ? "selectedGraphElement" : strength === "secondary" ? "secondarySelectedGraphElement" : "" }

function edgeSelection(snapshot: GraphViewSnapshot, edge: GraphEdge): Maybe<GraphSelectionStrength> {
  return snapshot.selectedEdge !== undefined && snapshot.selectedEdge.source === edge.source && snapshot.selectedEdge.label === edge.label ? snapshot.selectedEdge.strength : undefined }

class GraphLayoutState {
  positions = new Map<ID, Point>()
  velocities = new Map<ID, Point>()

  sync(nodes: GraphNode[]) {
    const ids = new Set(nodes.map(node => node.id))
    nodes.forEach((node, index) => {
      if (!this.positions.has(node.id)) this.positions.set(node.id, this.positions.size === 0 && index === 0 ? {x: 0, y: 0} : deterministicPosition(node.id, index))
      if (!this.velocities.has(node.id)) this.velocities.set(node.id, {x: 0, y: 0}) })
    Array.from(this.positions.keys()).forEach(id => { if (!ids.has(id)) this.positions.delete(id) })
    Array.from(this.velocities.keys()).forEach(id => { if (!ids.has(id)) this.velocities.delete(id) }) }

  step(snapshot: GraphViewSnapshot, dragging: Maybe<ID>) {
    this.sync(snapshot.nodes)
    let forces = new Map<ID, Point>(snapshot.nodes.map(node => [node.id, {x: 0, y: 0}]))
    for (let i = 0; i < snapshot.nodes.length; i++) {
      for (let j = i + 1; j < snapshot.nodes.length; j++) {
        const a = snapshot.nodes[i].id
        const b = snapshot.nodes[j].id
        const pa = this.positions.get(a)
        const pb = this.positions.get(b)
        if (!pa || !pb) continue
        const delta = sub(pa, pb)
        const force = scale(normalize(delta), Math.min(repulsionK / Math.max(lengthSq(delta), 1), maxForce))
        forces.set(a, add(forces.get(a)!, force))
        forces.set(b, sub(forces.get(b)!, force)) }}

    snapshot.edges.forEach(edge => {
      const pa = this.positions.get(edge.source)
      const pb = this.positions.get(edge.target)
      if (!pa || !pb) return
      const delta = sub(pb, pa)
      const dist = Math.max(length(delta), 0.1)
      const force = scale(normalize(delta), Math.max(-maxForce, Math.min(maxForce, attractionK * (dist - restLength))))
      forces.set(edge.source, add(forces.get(edge.source)!, force))
      forces.set(edge.target, sub(forces.get(edge.target)!, force)) })

    snapshot.nodes.forEach(node => {
      if (dragging === node.id) return
      const pos = this.positions.get(node.id)!
      const force = add(forces.get(node.id)!, scale(pos, -gravityK))
      const velocity = scale(add(this.velocities.get(node.id)!, force), damping)
      this.velocities.set(node.id, velocity)
      this.positions.set(node.id, add(pos, velocity)) }) }}

export class GraphViewComponent extends React.Component<{snapshot: GraphViewSnapshot, setGraphSelection: (selection: Maybe<GraphSelection>) => void, chooseID: (id: ID) => boolean}, {}> {
  svg: SVGSVGElement | null
  animationFrame: Maybe<number>
  layout = new GraphLayoutState()
  pan: Point = {x: 0, y: 0}
  zoom = 1
  dragging: Maybe<ID>
  dragOffset: Point = {x: 0, y: 0}
  panning = false
  lastPointer: Maybe<Point>
  dragMoved = false
  listeningToWindow = false

  componentDidMount() { this.animate() }
  componentWillUnmount() {
    maybe(this.animationFrame, () => {}, animationFrame => cancelAnimationFrame(animationFrame))
    this.stopWindowDragListeners() }

  animate = () => {
    this.layout.step(this.props.snapshot, this.dragging)
    this.forceUpdate()
    this.animationFrame = requestAnimationFrame(this.animate) }

  bounds() {
    const rect = this.svg?.getBoundingClientRect()
    return {width: rect?.width ?? 600, height: rect?.height ?? 400} }

  center() {
    const {width, height} = this.bounds()
    return {x: width / 2, y: height / 2} }

  toScreen(pos: Point): Point { return add(scale(pos, this.zoom), add(this.center(), this.pan)) }
  toGraph(pos: Point): Point { return scale(sub(sub(pos, this.center()), this.pan), 1 / this.zoom) }

  pointerFromClient(clientX: number, clientY: number): Point {
    const rect = this.svg!.getBoundingClientRect()
    return {x: clientX - rect.left, y: clientY - rect.top} }

  pointer(e: React.MouseEvent<SVGElement>): Point {
    return this.pointerFromClient(e.clientX, e.clientY) }

  startWindowDragListeners() {
    if (!this.listeningToWindow) {
      window.addEventListener("mousemove", this.onWindowMouseMove)
      window.addEventListener("mouseup", this.onWindowMouseUp)
      this.listeningToWindow = true }}

  stopWindowDragListeners() {
    if (this.listeningToWindow) {
      window.removeEventListener("mousemove", this.onWindowMouseMove)
      window.removeEventListener("mouseup", this.onWindowMouseUp)
      this.listeningToWindow = false }}

  onWheel = (e: React.WheelEvent<SVGSVGElement>) => {
    e.stopPropagation()
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault()
      const cursor = this.pointer(e)
      const graphPos = this.toGraph(cursor)
      this.zoom = Math.max(0.1, Math.min(5, this.zoom * Math.exp(-e.deltaY * 0.002)))
      this.pan = add(this.pan, sub(cursor, this.toScreen(graphPos))) }
    else {
      this.pan = add(this.pan, {x: -e.deltaX, y: -e.deltaY}) }
    this.forceUpdate() }

  onBackgroundMouseDown = (e: React.MouseEvent<SVGSVGElement>) => {
    e.stopPropagation()
    this.panning = true
    this.dragMoved = false
    this.lastPointer = this.pointer(e)
    this.startWindowDragListeners() }

  onNodeMouseDown(e: React.MouseEvent<SVGGElement>, id: ID) {
    if (chooseIDModifier(e)) {
      e.stopPropagation()
      e.preventDefault()
      return }
    e.stopPropagation()
    this.dragging = id
    this.dragMoved = false
    this.dragOffset = sub(this.toScreen(this.layout.positions.get(id)!), this.pointer(e))
    this.lastPointer = this.pointer(e)
    this.startWindowDragListeners() }

  onNodeClick(e: React.MouseEvent<SVGGElement>, id: ID) {
    if (chooseIDModifier(e)) {
      e.stopPropagation()
      e.preventDefault()
      this.props.chooseID(id) }}

  dragTo(pointer: Point) {
    if (!this.dragging && !this.panning) return
    if (this.lastPointer && length(sub(pointer, this.lastPointer)) > 2) this.dragMoved = true
    if (this.dragging) {
      this.layout.positions.set(this.dragging, this.toGraph(add(pointer, this.dragOffset)))
      this.layout.velocities.set(this.dragging, {x: 0, y: 0}) }
    else if (this.panning && this.lastPointer) {
      this.pan = add(this.pan, sub(pointer, this.lastPointer)) }
    this.lastPointer = pointer
    this.forceUpdate() }

  finishDrag() {
    if (this.dragging && !this.dragMoved) this.props.setGraphSelection({kind: "node", id: this.dragging})
    else if (this.panning && !this.dragMoved) this.props.setGraphSelection(undefined)
    this.dragging = undefined
    this.panning = false
    this.lastPointer = undefined
    this.stopWindowDragListeners() }

  onMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    e.stopPropagation()
    this.dragTo(this.pointer(e)) }

  onWindowMouseMove = (e: MouseEvent) => {
    this.dragTo(this.pointerFromClient(e.clientX, e.clientY)) }

  onWindowMouseUp = () => {
    this.finishDrag() }

  renderArrowhead(tip: Point, dir: Point, selected: Maybe<GraphSelectionStrength>, key: string) {
    const norm = normalize(dir)
    const perp = {x: -norm.y, y: norm.x}
    const size = 6 * this.zoom
    const width = 3 * this.zoom
    const p1 = add(sub(tip, scale(norm, size)), scale(perp, width))
    const p2 = add(sub(tip, scale(norm, size)), scale(perp, -width))
    return <polyline key={key} points={`${p1.x},${p1.y} ${tip.x},${tip.y} ${p2.x},${p2.y}`} className={["graphArrow", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} /> }

  renderGraphLabel(label: GraphLabel, x: number, y: number, iconSize: number) {
    const partWidths = label.parts.map(part => graphLabelPartWidth(part, iconSize) * this.zoom)
    const gap = labelPartGap * this.zoom
    let partX = x - (partWidths.reduce((a, b) => a + b, 0) + Math.max(0, label.parts.length - 1) * gap) / 2
    return <g>
      {label.parts.map((part, index) => {
        const width = partWidths[index]
        const className = index < label.parts.length - 1 ? "graphLabelType" : ""
        const element = maybe(part.name,
          () => maybe(part.guid,
            () => null,
            guid => <IdenticonComponent guid={guid} size={iconSize * this.zoom} x={partX} y={y - iconSize * this.zoom / 2} className={className} />),
          name => <text className={className} x={partX + width / 2} y={y + textFontSize * this.zoom * 0.35} fontSize={textFontSize * this.zoom}>{truncateLabel(name)}</text>)
        partX += width + gap
        return <React.Fragment key={index}>{element}</React.Fragment> })}
    </g> }

  renderEdgeLabel(edge: GraphEdge, pos: Point, selected: Maybe<GraphSelectionStrength>, key: string) {
    const size = edgeLabelSize(edge)
    const screenSize = scale(size, this.zoom)
    return <g key={key} className={["graphEdgeLabel", graphSelectionClass(selected)].filter(x => x !== "").join(" ")}
      onMouseDown={e => {
        e.stopPropagation()
        if (chooseIDModifier(e)) {
          e.preventDefault() } }}
      onClick={e => {
        e.stopPropagation()
        if (chooseIDModifier(e)) {
          e.preventDefault()
          this.props.chooseID(edge.label)
          return }
        this.props.setGraphSelection({kind: "edge", source: edge.source, label: edge.label}) }}>
      <rect x={pos.x - screenSize.x / 2} y={pos.y - screenSize.y / 2} width={screenSize.x} height={screenSize.y} />
      {this.renderGraphLabel(edge.labelText, pos.x, pos.y, edgeLabelIconSize)}
    </g> }

  renderEdges(nodeByID: Map<ID, GraphNode>) {
    let pairCounts = new Map<string, number>()
    this.props.snapshot.edges.forEach(edge => pairCounts.set(canonicalPair(edge.source, edge.target), (pairCounts.get(canonicalPair(edge.source, edge.target)) ?? 0) + 1))
    let pairIndices = new Map<string, number>()
    return this.props.snapshot.edges.flatMap((edge, index) => {
      const source = this.layout.positions.get(edge.source)
      const target = this.layout.positions.get(edge.target)
      const sourceNode = nodeByID.get(edge.source)
      const targetNode = nodeByID.get(edge.target)
      if (!source || !target || !sourceNode || !targetNode) return []
      const selected = edgeSelection(this.props.snapshot, edge)
      const sourceScreen = this.toScreen(source)
      const targetScreen = this.toScreen(target)
      const sourceHalf = scale(nodeSize(sourceNode), this.zoom / 2)
      const targetHalf = scale(nodeSize(targetNode), this.zoom / 2)
      const pairKey = canonicalPair(edge.source, edge.target)
      const total = pairCounts.get(pairKey) ?? 1
      const edgeIndex = pairIndices.get(pairKey) ?? 0
      pairIndices.set(pairKey, edgeIndex + 1)
      const curveOffset = (edgeIndex - (total - 1) / 2) * 25 * this.zoom

      if (edge.source === edge.target) {
        const loopHeight = (nodeIconSize * 2.5 + edgeIndex * 20) * this.zoom
        const loopWidth = (nodeIconSize * 1.5 + edgeIndex * 8) * this.zoom
        const cp1 = add(sourceScreen, {x: -loopWidth, y: -loopHeight})
        const cp2 = add(sourceScreen, {x: loopWidth, y: -loopHeight})
        const start = clipToRect(sourceScreen, sourceHalf, cp1)
        const end = clipToRect(sourceScreen, sourceHalf, cp2)
        const labelPos = pointOnCubic(start, cp1, cp2, end, 0.5)
        return [
          <path key={`edge-${index}`} d={`M ${start.x} ${start.y} C ${cp1.x} ${cp1.y}, ${cp2.x} ${cp2.y}, ${end.x} ${end.y}`} className={["graphEdge", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} />,
          this.renderArrowhead(end, sub(end, cp2), selected, `arrow-${index}`),
          this.renderEdgeLabel(edge, labelPos, selected, `label-${index}`)] }

      const mid = add(sourceScreen, scale(sub(targetScreen, sourceScreen), 0.5))
      const canonicalDir = idKey(edge.source) <= idKey(edge.target) ? normalize(sub(targetScreen, sourceScreen)) : normalize(sub(sourceScreen, targetScreen))
      const perp = {x: -canonicalDir.y, y: canonicalDir.x}
      const control = add(mid, scale(perp, curveOffset))
      const end = clipToRectToward(targetScreen, targetHalf, control, sourceScreen)
      const labelPos = pointOnQuadratic(sourceScreen, control, end, 0.5)
      return [
        <path key={`edge-${index}`} d={`M ${sourceScreen.x} ${sourceScreen.y} Q ${control.x} ${control.y}, ${end.x} ${end.y}`} className={["graphEdge", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} />,
        this.renderArrowhead(end, scale(sub(end, control), 2), selected, `arrow-${index}`),
        this.renderEdgeLabel(edge, labelPos, selected, `label-${index}`)] }) }

  renderNode(node: GraphNode) {
    const pos = this.layout.positions.get(node.id)
    if (!pos) return null
    const screen = this.toScreen(pos)
    const selected = this.props.snapshot.selectedNode !== undefined && this.props.snapshot.selectedNode.id === node.id ? this.props.snapshot.selectedNode.strength : undefined
    const className = ["graphNode", node.root ? "rootGraphNode" : "", graphSelectionClass(selected)].filter(x => x !== "").join(" ")
    const size = scale(nodeSize(node), this.zoom)
    return <g key={idKey(node.id)} className={className} onMouseDown={e => this.onNodeMouseDown(e, node.id)} onClick={e => this.onNodeClick(e, node.id)}>
      <rect x={screen.x - size.x / 2} y={screen.y - size.y / 2} width={size.x} height={size.y} rx={4 * this.zoom} />
      {this.renderGraphLabel(node.label, screen.x, screen.y, labelIconSize)}
    </g> }

  render() {
    this.layout.sync(this.props.snapshot.nodes)
    const nodeByID = new Map(this.props.snapshot.nodes.map(node => [node.id, node]))
    return <svg ref={svg => { this.svg = svg }} className="graphView"
      onMouseDown={this.onBackgroundMouseDown}
      onMouseMove={this.onMouseMove}
      onWheel={this.onWheel}
      onClick={e => e.stopPropagation()}>
      <rect className="graphBackground" x={0} y={0} width="100%" height="100%" />
      {this.renderEdges(nodeByID)}
      {this.props.snapshot.nodes.map(node => this.renderNode(node))}
    </svg> }
}
