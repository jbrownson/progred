module Progred.Editor
  ( Editor (..)
  , Focus (..)
  , FocusState (..)
  , blurString
  , defaultFocusState
  , deleteEdge
  , deleteFocusedEdge
  , editString
  , focusEdge
  , focusString
  , setEdge
  , setFocus
  ) where

import Progred.Document
import Progred.Graph
import Progred.GraphContext
import Progred.MapGraph
import Puri.Widgets (LineEditSelection (..))

-- Focus is the focused spot by path from the document root. Occurrences
-- of a shared node are distinct because they are reached along different
-- paths.
data Focus = Focus
  { focusPath :: [UUID]
  , focusState :: FocusState
  }
  deriving (Eq, Show)

data FocusState = FocusState
  { focusStringSelection :: LineEditSelection
  }
  deriving (Eq, Show)

defaultFocusState :: FocusState
defaultFocusState =
  FocusState
    { focusStringSelection = LineEditSelection 0 0 False
    }

data Editor = Editor
  { editorDocument :: Document
  , editorFocus :: Maybe Focus
  }

-- Tools pair a graph edit with the sync that keeps focus (later: any
-- transient data attached to paths) truthful. Editing the graph any other way
-- means owning that coherence yourself. Touched edges drop the state
-- that crossed them.
setEdge :: Edge -> Value -> Editor -> Editor
setEdge edge value =
  editGraph (setEdgeValue edge value) . dropCrossing edge

deleteEdge :: Edge -> Editor -> Editor
deleteEdge edge =
  editGraph (deleteEdgeValue edge) . dropCrossing edge

setFocus :: Maybe Focus -> Editor -> Editor
setFocus focus editor =
  editor {editorFocus = focus}

focusString :: [UUID] -> LineEditSelection -> Editor -> Editor
focusString path selection editor =
  setFocus (Just (Focus path state)) editor
  where
    state = (stateForPath path (editorFocus editor)) {focusStringSelection = selection}

focusEdge :: [UUID] -> Editor -> Editor
focusEdge path editor =
  setFocus (Just (Focus path (stateForPath path (editorFocus editor)))) editor

blurString :: [UUID] -> Editor -> Editor
blurString path editor =
  case editorFocus editor of
    Just focus | focusPath focus == path -> setFocus Nothing editor
    _ -> editor

-- Writing the edge drops focus that crossed it, so string edits pair the
-- graph write and the replacement selection in one operation.
editString :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
editString path string selection editor =
  case pathEdge (editorContext editor) path of
    Nothing -> editor
    Just edge ->
      (focusString path selection . setEdge edge (VString string)) editor

deleteFocusedEdge :: Editor -> Editor
deleteFocusedEdge editor =
  case editorFocus editor of
    Just focus -> deletePathEdge (focusPath focus) editor
    _ -> editor

deletePathEdge :: [UUID] -> Editor -> Editor
deletePathEdge path editor =
  case pathEdge (editorContext editor) path of
    Nothing -> editor
    Just edge -> deleteEdge edge editor

editGraph :: (MapGraph -> MapGraph) -> Editor -> Editor
editGraph change editor =
  editor {editorDocument = document {documentGraph = change (documentGraph document)}}
  where
    document = editorDocument editor

editorContext :: Editor -> GraphContext
editorContext editor =
  documentContext (editorDocument editor) []

-- Drops focus if its path crosses the touched edge or no longer
-- resolves at all.
dropCrossing :: Edge -> Editor -> Editor
dropCrossing edge editor =
  editor {editorFocus = kept =<< editorFocus editor}
  where
    kept focus = do
      PathWalk {walkedNodes = nodes} <- walkPath (editorContext editor) (focusPath focus)
      if edge `elem` zipWith Edge nodes (focusPath focus)
        then Nothing
        else Just focus

stateForPath :: [UUID] -> Maybe Focus -> FocusState
stateForPath path maybeFocus =
  case maybeFocus of
    Just focus | focusPath focus == path -> focusState focus
    _ -> defaultFocusState
