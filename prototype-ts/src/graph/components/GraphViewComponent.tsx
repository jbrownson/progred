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
const edgeLabelIconSize = 20
const edgeLabelInnerIconSize = 16
const edgeLabelPadding = 4
const labelPartGap = 4
const textFont = `500 ${textFontSize}px sans-serif`
let textMeasureContext: CanvasRenderingContext2D | null = null
let textWidthCache = new Map<string, number>()
let nextGraphViewID = 0

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

type GraphViewProps = {snapshot: GraphViewSnapshot, setGraphSelection: (selection: Maybe<GraphSelection>) => void, chooseID: (id: ID) => boolean}

export function GraphViewComponent(props: GraphViewProps) {
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const propsRef = React.useRef(props)
  propsRef.current = props
  const clipIDPrefix = React.useRef(`graphView${nextGraphViewID++}`).current
  const svg = React.useRef<SVGSVGElement | null>(null)
  const animationFrame = React.useRef<Maybe<number>>(undefined)
  const layout = React.useRef(new GraphLayoutState()).current
  const pan = React.useRef<Point>({x: 0, y: 0})
  const zoom = React.useRef(1)
  const dragging = React.useRef<Maybe<ID>>(undefined)
  const dragOffset = React.useRef<Point>({x: 0, y: 0})
  const panning = React.useRef(false)
  const lastPointer = React.useRef<Maybe<Point>>(undefined)
  const dragMoved = React.useRef(false)
  const listeningToWindow = React.useRef(false)
  const windowHandlers = React.useRef({
    mouseMove: (_e: MouseEvent) => {},
    mouseUp: () => {} })

  const onWindowMouseMove = React.useCallback((e: MouseEvent) => windowHandlers.current.mouseMove(e), [])
  const onWindowMouseUp = React.useCallback(() => windowHandlers.current.mouseUp(), [])

  function bounds() {
    const rect = svg.current?.getBoundingClientRect()
    return {width: rect?.width ?? 600, height: rect?.height ?? 400} }

  function center() {
    const {width, height} = bounds()
    return {x: width / 2, y: height / 2} }

  function toScreen(pos: Point): Point { return add(scale(pos, zoom.current), add(center(), pan.current)) }
  function toGraph(pos: Point): Point { return scale(sub(sub(pos, center()), pan.current), 1 / zoom.current) }

  function pointerFromClient(clientX: number, clientY: number): Point {
    const rect = svg.current!.getBoundingClientRect()
    return {x: clientX - rect.left, y: clientY - rect.top} }

  function pointer(e: React.MouseEvent<SVGElement>): Point {
    return pointerFromClient(e.clientX, e.clientY) }

  function stopWindowDragListeners() {
    if (listeningToWindow.current) {
      window.removeEventListener("mousemove", onWindowMouseMove)
      window.removeEventListener("mouseup", onWindowMouseUp)
      listeningToWindow.current = false }}

  function startWindowDragListeners() {
    if (!listeningToWindow.current) {
      window.addEventListener("mousemove", onWindowMouseMove)
      window.addEventListener("mouseup", onWindowMouseUp)
      listeningToWindow.current = true }}

  function dragTo(pointer: Point) {
    if (!dragging.current && !panning.current) return
    if (lastPointer.current && length(sub(pointer, lastPointer.current)) > 2) dragMoved.current = true
    if (dragging.current) {
      layout.positions.set(dragging.current, toGraph(add(pointer, dragOffset.current)))
      layout.velocities.set(dragging.current, {x: 0, y: 0}) }
    else if (panning.current && lastPointer.current) {
      pan.current = add(pan.current, sub(pointer, lastPointer.current)) }
    lastPointer.current = pointer
    forceUpdate() }

  function finishDrag() {
    if (dragging.current && !dragMoved.current) propsRef.current.setGraphSelection({kind: "node", id: dragging.current})
    else if (panning.current && !dragMoved.current) propsRef.current.setGraphSelection(undefined)
    dragging.current = undefined
    panning.current = false
    lastPointer.current = undefined
    stopWindowDragListeners() }

  windowHandlers.current.mouseMove = e => dragTo(pointerFromClient(e.clientX, e.clientY))
  windowHandlers.current.mouseUp = () => finishDrag()

  React.useEffect(() => {
    function animate() {
      layout.step(propsRef.current.snapshot, dragging.current)
      forceUpdate()
      animationFrame.current = requestAnimationFrame(animate) }
    animate()
    return () => {
      maybe(animationFrame.current, () => {}, animationFrame => cancelAnimationFrame(animationFrame))
      stopWindowDragListeners() } }, [])

  function onWheel(e: React.WheelEvent<SVGSVGElement>) {
    e.stopPropagation()
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault()
      const cursor = pointer(e)
      const graphPos = toGraph(cursor)
      zoom.current = Math.max(0.1, Math.min(5, zoom.current * Math.exp(-e.deltaY * 0.004)))
      pan.current = add(pan.current, sub(cursor, toScreen(graphPos))) }
    else {
      pan.current = add(pan.current, {x: -e.deltaX, y: -e.deltaY}) }
    forceUpdate() }

  function onBackgroundMouseDown(e: React.MouseEvent<SVGSVGElement>) {
    e.stopPropagation()
    panning.current = true
    dragMoved.current = false
    lastPointer.current = pointer(e)
    startWindowDragListeners() }

  function onNodeMouseDown(e: React.MouseEvent<SVGGElement>, id: ID) {
    if (chooseIDModifier(e)) {
      e.stopPropagation()
      e.preventDefault()
      return }
    e.stopPropagation()
    dragging.current = id
    dragMoved.current = false
    dragOffset.current = sub(toScreen(layout.positions.get(id)!), pointer(e))
    lastPointer.current = pointer(e)
    startWindowDragListeners() }

  function onNodeClick(e: React.MouseEvent<SVGGElement>, id: ID) {
    if (chooseIDModifier(e)) {
      e.stopPropagation()
      e.preventDefault()
      props.chooseID(id) }}

  function onMouseMove(e: React.MouseEvent<SVGSVGElement>) {
    e.stopPropagation()
    dragTo(pointer(e)) }

  function renderArrowhead(tip: Point, dir: Point, selected: Maybe<GraphSelectionStrength>, key: string) {
    const norm = normalize(dir)
    const perp = {x: -norm.y, y: norm.x}
    const size = 6 * zoom.current
    const width = 3 * zoom.current
    const p1 = add(sub(tip, scale(norm, size)), scale(perp, width))
    const p2 = add(sub(tip, scale(norm, size)), scale(perp, -width))
    return <polyline key={key} points={`${p1.x},${p1.y} ${tip.x},${tip.y} ${p2.x},${p2.y}`} className={["graphArrow", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} /> }

  function renderGraphLabel(label: GraphLabel, x: number, y: number, iconSize: number, edgeLabelClipPrefix: Maybe<string> = undefined) {
    const partWidths = label.parts.map(part => graphLabelPartWidth(part, iconSize) * zoom.current)
    const gap = labelPartGap * zoom.current
    let partX = x - (partWidths.reduce((a, b) => a + b, 0) + Math.max(0, label.parts.length - 1) * gap) / 2
    return <g>
      {label.parts.map((part, index) => {
        const width = partWidths[index]
        const className = index < label.parts.length - 1 ? "graphLabelType" : ""
        const clipID = `${clipIDPrefix}-${edgeLabelClipPrefix}-${index}`
        const element = maybe(part.name,
          () => maybe(part.guid,
            () => null,
            guid => edgeLabelClipPrefix
              ? <g className={["graphEdgeIdenticon", className].filter(x => x !== "").join(" ")}>
                <clipPath id={clipID}>
                  <circle cx={partX + width / 2} cy={y} r={edgeLabelIconSize * zoom.current / 2} />
                </clipPath>
                <circle cx={partX + width / 2} cy={y} r={edgeLabelIconSize * zoom.current / 2} />
                <g clipPath={`url(#${clipID})`}>
                  <IdenticonComponent guid={guid} size={edgeLabelInnerIconSize * zoom.current} x={partX + (width - edgeLabelInnerIconSize * zoom.current) / 2} y={y - edgeLabelInnerIconSize * zoom.current / 2} />
                </g>
              </g>
              : <IdenticonComponent guid={guid} size={iconSize * zoom.current} x={partX} y={y - iconSize * zoom.current / 2} className={className} />),
          name => <text className={className} x={partX + width / 2} y={y + textFontSize * zoom.current * 0.35} fontSize={textFontSize * zoom.current}>{truncateLabel(name)}</text>)
        partX += width + gap
        return <React.Fragment key={index}>{element}</React.Fragment> })}
    </g> }

  function renderEdgeLabel(edge: GraphEdge, pos: Point, selected: Maybe<GraphSelectionStrength>, key: string) {
    const size = edgeLabelSize(edge)
    const screenSize = scale(size, zoom.current)
    return <g key={key} className={["graphEdgeLabel", graphSelectionClass(selected)].filter(x => x !== "").join(" ")}
      onMouseDown={e => {
        e.stopPropagation()
        if (chooseIDModifier(e)) {
          e.preventDefault() } }}
      onClick={e => {
        e.stopPropagation()
        if (chooseIDModifier(e)) {
          e.preventDefault()
          props.chooseID(edge.label)
          return }
        props.setGraphSelection({kind: "edge", source: edge.source, label: edge.label}) }}>
      <rect x={pos.x - screenSize.x / 2} y={pos.y - screenSize.y / 2} width={screenSize.x} height={screenSize.y} />
      {renderGraphLabel(edge.labelText, pos.x, pos.y, edgeLabelIconSize, key)}
    </g> }

  function renderEdges(nodeByID: Map<ID, GraphNode>) {
    let pairCounts = new Map<string, number>()
    props.snapshot.edges.forEach(edge => pairCounts.set(canonicalPair(edge.source, edge.target), (pairCounts.get(canonicalPair(edge.source, edge.target)) ?? 0) + 1))
    let pairIndices = new Map<string, number>()
    return props.snapshot.edges.flatMap((edge, index) => {
      const source = layout.positions.get(edge.source)
      const target = layout.positions.get(edge.target)
      const sourceNode = nodeByID.get(edge.source)
      const targetNode = nodeByID.get(edge.target)
      if (!source || !target || !sourceNode || !targetNode) return []
      const selected = edgeSelection(props.snapshot, edge)
      const sourceScreen = toScreen(source)
      const targetScreen = toScreen(target)
      const sourceHalf = scale(nodeSize(sourceNode), zoom.current / 2)
      const targetHalf = scale(nodeSize(targetNode), zoom.current / 2)
      const pairKey = canonicalPair(edge.source, edge.target)
      const total = pairCounts.get(pairKey) ?? 1
      const edgeIndex = pairIndices.get(pairKey) ?? 0
      pairIndices.set(pairKey, edgeIndex + 1)
      const curveOffset = (edgeIndex - (total - 1) / 2) * 25 * zoom.current

      if (edge.source === edge.target) {
        const loopHeight = (nodeIconSize * 2.5 + edgeIndex * 20) * zoom.current
        const loopWidth = (nodeIconSize * 1.5 + edgeIndex * 8) * zoom.current
        const cp1 = add(sourceScreen, {x: -loopWidth, y: -loopHeight})
        const cp2 = add(sourceScreen, {x: loopWidth, y: -loopHeight})
        const start = clipToRect(sourceScreen, sourceHalf, cp1)
        const end = clipToRect(sourceScreen, sourceHalf, cp2)
        const labelPos = pointOnCubic(start, cp1, cp2, end, 0.5)
        return [
          <path key={`edge-${index}`} d={`M ${start.x} ${start.y} C ${cp1.x} ${cp1.y}, ${cp2.x} ${cp2.y}, ${end.x} ${end.y}`} className={["graphEdge", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} />,
          renderArrowhead(end, sub(end, cp2), selected, `arrow-${index}`),
          renderEdgeLabel(edge, labelPos, selected, `label-${index}`)] }

      const mid = add(sourceScreen, scale(sub(targetScreen, sourceScreen), 0.5))
      const canonicalDir = idKey(edge.source) <= idKey(edge.target) ? normalize(sub(targetScreen, sourceScreen)) : normalize(sub(sourceScreen, targetScreen))
      const perp = {x: -canonicalDir.y, y: canonicalDir.x}
      const control = add(mid, scale(perp, curveOffset))
      const end = clipToRectToward(targetScreen, targetHalf, control, sourceScreen)
      const labelPos = pointOnQuadratic(sourceScreen, control, end, 0.5)
      return [
        <path key={`edge-${index}`} d={`M ${sourceScreen.x} ${sourceScreen.y} Q ${control.x} ${control.y}, ${end.x} ${end.y}`} className={["graphEdge", graphSelectionClass(selected)].filter(x => x !== "").join(" ")} />,
        renderArrowhead(end, scale(sub(end, control), 2), selected, `arrow-${index}`),
        renderEdgeLabel(edge, labelPos, selected, `label-${index}`)] }) }

  function renderNode(node: GraphNode) {
    const pos = layout.positions.get(node.id)
    if (!pos) return null
    const screen = toScreen(pos)
    const selected = props.snapshot.selectedNode !== undefined && props.snapshot.selectedNode.id === node.id ? props.snapshot.selectedNode.strength : undefined
    const className = ["graphNode", node.root ? "rootGraphNode" : "", graphSelectionClass(selected)].filter(x => x !== "").join(" ")
    const size = scale(nodeSize(node), zoom.current)
    return <g key={idKey(node.id)} className={className} onMouseDown={e => onNodeMouseDown(e, node.id)} onClick={e => onNodeClick(e, node.id)}>
      <rect x={screen.x - size.x / 2} y={screen.y - size.y / 2} width={size.x} height={size.y} rx={4 * zoom.current} />
      {renderGraphLabel(node.label, screen.x, screen.y, labelIconSize)}
    </g> }

  layout.sync(props.snapshot.nodes)
  const nodeByID = new Map(props.snapshot.nodes.map(node => [node.id, node]))
  return <svg ref={element => { svg.current = element }} className="graphView"
    onMouseDown={onBackgroundMouseDown}
    onMouseMove={onMouseMove}
    onWheel={onWheel}
    onClick={e => e.stopPropagation()}>
    <rect className="graphBackground" x={0} y={0} width="100%" height="100%" />
    {renderEdges(nodeByID)}
    {props.snapshot.nodes.map(node => renderNode(node))}
  </svg>
}
