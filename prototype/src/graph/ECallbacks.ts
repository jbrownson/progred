import { mapMaybe, Maybe, nothing } from "../lib/Maybe"
import { cursorsEqual } from "./Cursor"
import { _delete, _get, environment, set } from "./Environment"
import { GUID, ID } from "./ID"
import { _Selection } from "./Selection"
import { UndoRedo } from "./UndoRedo"

export type ECallbacks = {
  onGet(id: ID, label: ID): void
  onEdges(id: ID): void
  willSet(guid: GUID, label: ID, to: ID): void
  willDelete(guid: GUID, label: ID): void
  onGetSelection(): void
  willSetSelection(selection: Maybe<_Selection>): void }

export const noopECallbacks = {
  onGet: () => {},
  onEdges: () => {},
  willSet: () => {},
  willDelete: () => {},
  onGetSelection: () => {},
  willSetSelection: () => {} }

export type ReadLog = {
  gets: {id: ID, label: ID}[]
  edges: ID[]
  gotSelection: boolean }
export class ReadOnlyViolation {}
export function readOnlyECallbacks(): {readLog: ReadLog, eCallbacks: ECallbacks} {
  let readLog: ReadLog = { gets: [], edges: [], gotSelection: false }
  return {readLog, eCallbacks: {
    onGet: (id, label) => readLog.gets.push({id, label}),
    onEdges: id => readLog.edges.push(id),
    willSet: () => {throw new ReadOnlyViolation},
    willDelete: () => {throw new ReadOnlyViolation},
    onGetSelection: () => {readLog.gotSelection = true},
    willSetSelection: () => {throw new ReadOnlyViolation} }}}

export function undoRedoECallbacks(): {undoRedoArray: UndoRedo[], eCallbacks: ECallbacks} {
    let undoRedoArray: UndoRedo[] = []
    return {undoRedoArray, eCallbacks:{
    onGet: () => {},
    onEdges: () => {},
    willSet: (guid: GUID, label: ID, to: ID) => {
      const prevId = _get(guid, label)
      undoRedoArray.push(new UndoRedo(
        () => prevId === nothing ? _delete(guid, label) : set(guid, label, prevId),
        () => set(guid, label, to),
        false ))},
    willDelete: (guid: GUID, label: ID) => {
      mapMaybe(
        _get(guid, label),
        prevId => {
          undoRedoArray.push(
            new UndoRedo(
              () => set(guid, label, prevId),
              () => _delete(guid, label),
              false ))})},
    onGetSelection: () => {},
    willSetSelection: (nextSelection: Maybe<_Selection>) => {
      const prevSelection = environment().selection
      const equal = prevSelection === nextSelection || (prevSelection && nextSelection && cursorsEqual(prevSelection.cursor, nextSelection.cursor))
      if (!equal) {
        undoRedoArray.push(
          new UndoRedo(
            () => environment().selection = prevSelection,
            () => environment().selection = nextSelection,
            true))}} }}}

export function composeECallbacks(lhs: ECallbacks, rhs: ECallbacks): ECallbacks {
  return {
    onGet: (id: ID, label: ID) => { lhs.onGet(id, label); rhs.onGet(id, label) },
    onEdges: (id: ID) => { lhs.onEdges(id); rhs.onEdges(id); },
    willSet: (guid: GUID, label: ID, to: ID) => { lhs.willSet(guid, label, to); rhs.willSet(guid, label, to) },
    willDelete: (guid: GUID, label: ID) => { lhs.willDelete(guid, label); rhs.willDelete(guid, label) },
    onGetSelection: () => { lhs.onGetSelection(); rhs.onGetSelection() },
    willSetSelection: (selection: Maybe<_Selection>) => { lhs.willSetSelection(selection); rhs.willSetSelection(selection) } }}