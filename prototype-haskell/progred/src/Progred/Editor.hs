module Progred.Editor
  ( Editor (..)
  , Focus (..)
  , FocusState (..)
  , NumberEdit (..)
  , PendingEdit (..)
  , blurValue
  , blurString
  , cancelPending
  , defaultFocusState
  , deleteEdge
  , deleteFocusedEdge
  , editFloat
  , editInt
  , editString
  , focusEdge
  , focusNumber
  , focusPending
  , insertStringEdge
  , focusString
  , insertListString
  , parseFloatValue
  , parseIntValue
  , setEdge
  , setFocus
  , spliceListItem
  ) where

import qualified Data.Map.Strict as Map
import Text.Read (readMaybe)
import Progred.Builtins
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
  , focusNumberEdit :: Maybe NumberEdit
  , focusPendingEdit :: Maybe PendingEdit
  }
  deriving (Eq, Show)

data NumberEdit = NumberEdit
  { numberEditText :: String
  , numberEditSelection :: LineEditSelection
  }
  deriving (Eq, Show)

data PendingEdit = PendingEdit
  { pendingEditText :: String
  , pendingEditSelection :: LineEditSelection
  }
  deriving (Eq, Show)

defaultFocusState :: FocusState
defaultFocusState =
  FocusState
    { focusStringSelection = LineEditSelection 0 0 False
    , focusNumberEdit = Nothing
    , focusPendingEdit = Nothing
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

focusNumber :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
focusNumber path string selection editor =
  setFocus (Just (Focus path state)) editor
  where
    state = (stateForPath path (editorFocus editor)) {focusNumberEdit = Just (NumberEdit string selection)}

focusPending :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
focusPending path string selection editor =
  setFocus (Just (Focus path state)) editor
  where
    state =
      (stateForPath path (editorFocus editor))
        { focusPendingEdit = Just (PendingEdit string selection)
        }

blurValue :: [UUID] -> Editor -> Editor
blurValue path editor =
  case editorFocus editor of
    Just focus | focusPath focus == path -> setFocus Nothing editor
    _ -> editor

blurString :: [UUID] -> Editor -> Editor
blurString =
  blurValue

cancelPending :: [UUID] -> Editor -> Editor
cancelPending =
  blurValue

-- Writing the edge drops focus that crossed it, so string edits pair the
-- graph write and the replacement selection in one operation.
editString :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
editString path string selection editor =
  case pathEdge (editorContext editor) path of
    Nothing -> editor
    Just edge ->
      (focusString path selection . setEdge edge (VString string)) editor

editInt :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
editInt =
  editNumber parseIntValue

editFloat :: [UUID] -> String -> LineEditSelection -> Editor -> Editor
editFloat =
  editNumber parseFloatValue

parseIntValue :: String -> Maybe Value
parseIntValue string =
  VInt <$> (readMaybe string :: Maybe Integer)

parseFloatValue :: String -> Maybe Value
parseFloatValue string =
  VFloat <$> (readMaybe string :: Maybe Double)

editNumber :: (String -> Maybe Value) -> [UUID] -> String -> LineEditSelection -> Editor -> Editor
editNumber parse path string selection editor =
  case pathEdge (editorContext editor) path of
    Nothing -> editor
    Just edge ->
      case parse string of
        Just value -> (focusNumber path string selection . setEdge edge value) editor
        Nothing -> focusNumber path string selection editor

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

spliceListItem :: [UUID] -> Editor -> Editor
spliceListItem path editor =
  case listItemCellPath path of
    Nothing -> editor
    Just cellPath ->
      case (pathEdge context cellPath, resolvePath context cellPath) of
        (Just linkEdge, Just (VRef cellNode)) ->
          case lookupNode context cellNode >>= Map.lookup tailLabel of
            Just next -> setEdge linkEdge next editor
            Nothing -> editor
        _ -> editor
  where
    context = editorContext editor

insertListString :: [UUID] -> UUID -> String -> LineEditSelection -> Editor -> Editor
insertListString path newCell string selection editor =
  case (pathEdge context path, resolvePath context path) of
    (Just linkEdge, Just oldTail) ->
      ( focusString (path <> [headLabel]) selection
          . editGraph (Map.insert newCell (Map.fromList [(isaLabel, VRef listConsNode), (headLabel, VString string), (tailLabel, oldTail)]))
          . setEdge linkEdge (VRef newCell)
      )
        editor
    _ -> editor
  where
    context = editorContext editor

insertStringEdge :: [UUID] -> UUID -> String -> LineEditSelection -> Editor -> Editor
insertStringEdge parentPath label string selection editor =
  case resolvePath (editorContext editor) parentPath of
    Just (VRef parent) ->
      ( focusString (parentPath <> [label]) selection
          . setEdge (Edge parent label) (VString string)
      )
        editor
    _ -> editor

listItemCellPath :: [UUID] -> Maybe [UUID]
listItemCellPath path =
  case reverse path of
    label : reversedCellPath
      | label == headLabel -> Just (reverse reversedCellPath)
    _ -> Nothing

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
