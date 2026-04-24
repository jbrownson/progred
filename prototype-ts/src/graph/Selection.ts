import { Cursor } from "./Cursor"
import { NumberEditorState, PlaceholderState } from "./render/D"

export type _Selection = { cursor: Cursor } & NumberEditorState & PlaceholderState
