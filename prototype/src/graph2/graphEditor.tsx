import * as React from "react"
import * as ReactDOM from "react-dom"
import { generateGUID } from "../lib/generateGUID"
import { GUID } from "./graph/GUIDMap"
import { GUIDNode } from "./graph/GUIDNode"
import { MapGUIDMap } from "./graph/MapGUIDMap"
import { Node } from "./graph/Node"
import { StringNode } from "./graph/StringNode"
import { guidMap, withGUIDMap } from "./graph/withGUIDMap"

export let structStruct = new GUIDNode(generateGUID())
export let unionStruct = new GUIDNode(generateGUID())
export let fieldStruct = new GUIDNode(generateGUID())
export let nonemptyListStruct = new GUIDNode(generateGUID())
export let emptyListStruct = new GUIDNode(generateGUID())
export let atomicTypeStruct = new GUIDNode(generateGUID())
export let typeUnion = new GUIDNode(generateGUID())
export let listUnion = new GUIDNode(generateGUID())
export let structField = new GUIDNode(generateGUID())
export let nameField = new GUIDNode(generateGUID())
export let fieldsField = new GUIDNode(generateGUID())
export let typesField = new GUIDNode(generateGUID())
export let headField = new GUIDNode(generateGUID())
export let tailField = new GUIDNode(generateGUID())
export let typeField = new GUIDNode(generateGUID())
export let stringAtomicType = new GUIDNode(generateGUID())
export let numberAtomicType = new GUIDNode(generateGUID())

function newNode(guid: GUID, ...edges: {label: Node, node: Node}[]) {
  guidMap().sets(guid, edges)
  return new GUIDNode(guid) }

function listFromArray(array: Node[]): Node {
  return newNode(generateGUID(), ...array.length > 0
    ? [{label: structField, node: emptyListStruct}]
    : [{label: structField, node: nonemptyListStruct}, {label: headField, node: array[0]}, {label: tailField, node: listFromArray(array.slice(1))}] )}

function initStruct(struct: GUIDNode, name: string, fields: Node[]) {
  guidMap().sets(struct.guid, [{label: structField, node: structStruct}, {label: nameField, node: new StringNode(name)}, {label: fieldsField, node: listFromArray(fields)}]) }

function initUnion(union: GUIDNode, name: string, types: Node[]) {
  guidMap().sets(union.guid, [{label: structField, node: unionStruct}, {label: nameField, node: new StringNode(name)}, {label: typesField, node: listFromArray(types)}]) }

function initField(field: GUIDNode, name: string) {
  guidMap().sets(field.guid, [{label: structField, node: fieldStruct}, {label: nameField, node: new StringNode(name)}]) }

function initAtomicType(atomicType: GUIDNode, name: string) {
  guidMap().sets(atomicType.guid, [{label: structField, node: atomicTypeStruct}, {label: nameField, node: new StringNode(name)}]) }

let mapGUIDMap = new MapGUIDMap
withGUIDMap(mapGUIDMap, () => {
  initStruct(structStruct, "Struct", [nameField, fieldsField])
  initStruct(unionStruct, "Union", [nameField, typesField])
  initStruct(fieldStruct, "Field", [nameField])
  initStruct(nonemptyListStruct, "Nonempty List", [headField, tailField])
  initStruct(emptyListStruct, "Empty List", [])
  initStruct(atomicTypeStruct, "Atomic Type", [nameField])
  initUnion(typeUnion, "Type", [unionStruct, structStruct, atomicTypeStruct])
  initUnion(listUnion, "List", [nonemptyListStruct, emptyListStruct])
  initField(structField, "struct")
  initField(nameField, "name")
  initField(fieldsField, "fields")
  initField(typesField, "types")
  initField(headField, "head")
  initField(tailField, "tail")
  initField(typeField, "type")
  initAtomicType(stringAtomicType, "string")
  initAtomicType(numberAtomicType, "number") })

export class RootComponent extends React.Component<{}, {}> {
  panel: HTMLElement | null = null
  render(): JSX.Element {
    return <div style={{position: "absolute", top: 0, left: 0, right: 0, bottom: 0}}>
      <div ref={panel => this.panel = panel} style={{display: "inline-block", width: "100%", height: "100%", overflow: "scroll"}}>
        <div className="doc">
          asdf
        </div></div></div> } }

  export let rootComponent = ReactDOM.render(<RootComponent />, document.getElementById('root') as HTMLElement) as RootComponent