module Progred.Editor
  ( Editor (..)
  , Focus (..)
  , deleteEdge
  , editString
  , setEdge
  , setFocus
  ) where

import Progred.Document
import Progred.Graph
import Progred.MapGraph
import Puri.Widgets.LineEdit (LineEditState)

-- Focus is the focused spot: the label path from the document root and
-- the text-edit state at its target. Occurrences of a shared node are
-- distinct because they are reached along different paths.
data Focus
  = Focus [UUID] LineEditState
  deriving (Eq, Show)

data Editor = Editor
  { editorDocument :: Document
  , editorFocus :: Maybe Focus
  }

-- Tools pair a graph edit with the sync that keeps focus (later: any
-- state attached to paths) truthful. Editing the graph any other way
-- means owning that coherence yourself. Touched edges drop the state
-- that crossed them.
setEdge :: UUID -> UUID -> Value -> Editor -> Editor
setEdge source label value =
  editGraph (setEdgeValue source label value) . dropCrossing source label

deleteEdge :: UUID -> UUID -> Editor -> Editor
deleteEdge source label =
  editGraph (deleteEdgeValue source label) . dropCrossing source label

setFocus :: Maybe Focus -> Editor -> Editor
setFocus focus editor =
  editor {editorFocus = focus}

-- A line edit reports its whole desired state, so this writes the
-- string and places the text cursor in one step; writing the edge drops
-- the focus that crossed it, so the two must not be done separately.
editString :: [UUID] -> String -> Maybe LineEditState -> Editor -> Editor
editString path string maybeState editor =
  case target of
    Nothing -> editor
    Just (source, label) ->
      (setFocus (Focus path <$> maybeState) . setEdge source label (VString string)) editor
  where
    target = do
      (nodes, _) <- walkPath (editorDocument editor) path
      (,) <$> lastMaybe nodes <*> lastMaybe path

editGraph :: (MapGraph -> MapGraph) -> Editor -> Editor
editGraph change editor =
  editor {editorDocument = document {documentGraph = change (documentGraph document)}}
  where
    document = editorDocument editor

-- Drops focus if its path crosses the touched edge or no longer
-- resolves at all.
dropCrossing :: UUID -> UUID -> Editor -> Editor
dropCrossing source label editor =
  editor {editorFocus = kept =<< editorFocus editor}
  where
    kept focus@(Focus path _) = do
      (nodes, _) <- walkPath (editorDocument editor) path
      if (source, label) `elem` zip nodes path
        then Nothing
        else Just focus

lastMaybe :: [item] -> Maybe item
lastMaybe =
  foldl (\_ item -> Just item) Nothing
