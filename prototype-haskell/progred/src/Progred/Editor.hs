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
import Puri.Widgets (LineEditFocus (..), LineEditSelection)

-- Focus is the focused spot: the label path from the document root and
-- the text selection at its target. Occurrences of a shared node are
-- distinct because they are reached along different paths.
data Focus
  = Focus [UUID] LineEditSelection
  deriving (Eq, Show)

data Editor = Editor
  { editorDocument :: Document
  , editorFocus :: Maybe Focus
  }

-- Tools pair a graph edit with the sync that keeps focus (later: any
-- transient data attached to paths) truthful. Editing the graph any other way
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

-- A line edit reports its whole desired value, so this writes the
-- string and places the text selection in one step; writing the edge drops
-- the focus that crossed it, so the two must not be done separately.
editString :: [UUID] -> String -> LineEditFocus -> Editor -> Editor
editString path string lineFocus editor =
  case target of
    Nothing -> editor
    Just (source, label) ->
      (setFocus (focusAt path lineFocus) . setEdge source label (VString string)) editor
  where
    target = do
      (nodes, _) <- walkPath (editorDocument editor) path
      (,) <$> lastMaybe nodes <*> lastMaybe path

focusAt :: [UUID] -> LineEditFocus -> Maybe Focus
focusAt path lineFocus =
  case lineFocus of
    LineEditUnfocused -> Nothing
    LineEditFocused selection -> Just (Focus path selection)

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
