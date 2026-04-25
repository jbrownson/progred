import * as React from "react"
import { GUID } from "../model/ID"

const gridSize = 5

function hashString(s: string) {
  let hash = 2166136261
  for (let i = 0; i < s.length; i++)
    hash = Math.imul(hash ^ s.charCodeAt(i), 16777619)
  return hash >>> 0 }

function identiconCells(hash: number) {
  return Array.from({length: gridSize}, (_row, row) =>
    Array.from({length: gridSize}, (_col, col) => {
      const mirroredCol = Math.min(col, gridSize - 1 - col)
      const bitIndex = row * 3 + mirroredCol
      return ((hash >>> bitIndex) & 1) === 1 })) }

export function IdenticonComponent({guid, size = 16, x, y, className, style}: {guid: GUID, size?: number, x?: number, y?: number, className?: string, style?: React.CSSProperties}) {
  const hash = hashString(guid)
  const color = `hsl(${hash % 360} 65% 45%)`
  return <svg
    x={x}
    y={y}
    className={className}
    width={size}
    height={size}
    viewBox={`0 0 ${gridSize} ${gridSize}`}
    aria-label={guid}
    style={{verticalAlign: "-3px", ...style}}>
    <rect className="identiconBackground" x={0} y={0} width={gridSize} height={gridSize} rx={0.45} fill="#fafafa" stroke="#b4b4b4" strokeWidth={0.25} />
    {identiconCells(hash).map((row, y) => row.map((filled, x) =>
      filled ? <rect key={`${x},${y}`} x={x} y={y} width={1} height={1} fill={color} /> : null))}
  </svg> }
