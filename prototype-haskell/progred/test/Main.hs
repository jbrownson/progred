module Main
  ( main
  ) where

import qualified Data.Map.Strict as Map
import qualified Data.UUID.Types as UUID
import Control.Monad.Trans.State.Strict (State, execState, modify)
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.MapGraph (MapGraph)
import Progred.Projection
import Progred.Render.List
import Progred.Render.Raw
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widgets (LineEditSelection (..))
import Test.QuickCheck

main :: IO ()
main = do
  run "setEdge" (propToolTracksValue genSetEdge)
  run "deleteEdge" (propToolTracksValue genDeleteEdge)
  run "editString" propEditStringWritesAndFocuses
  run "editInt" propEditIntBuffersInvalidAndCommitsValid
  run "editFloat" propEditFloatBuffersInvalidAndCommitsValid
  run "blurString" propBlurStringOnlyClearsMatchingPath
  run "deleteFocusedEdge" propDeleteFocusedEdgeDeletesEdge
  run "graphContext" propGraphContextUsesLibraries
  run "listItemFocus" propListNodeItemFocusesListElement
  run "listItemDelete" propListNodeItemDeleteSplicesList
  where
    run name prop = do
      result <- quickCheckWithResult stdArgs {maxSuccess = 1000} prop
      case result of
        Success {} -> pure ()
        _ -> fail (name <> ": law failed")

-- Every tool must keep focus on the same value: write a sentinel at the
-- focused path's target, apply the tool, and surviving focus must still
-- address the sentinel (or have been dropped).
propToolTracksValue :: (MapGraph -> Gen (Editor -> Editor)) -> Property
propToolTracksValue genTool =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      tool <- genTool graph
      pure (graph, path, tool)
    check (graph, path, tool) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        case writeAt graph rootId path sentinel of
          Nothing -> counterexample "generated path did not resolve" False
          Just instrumented ->
            let edited = tool Editor {editorDocument = Document rootId instrumented, editorFocus = Just (Focus path (testFocusState restingState))}
             in case editorFocus edited of
                  Nothing -> property True
                  Just (Focus path' _) ->
                    counterexample
                      ("survived: " <> show path')
                      (readAt (documentGraph (editorDocument edited)) rootId path' == Just sentinel)

-- editString must write the string at the path and leave focus exactly
-- as the widget asked.
propEditStringWritesAndFocuses :: Property
propEditStringWritesAndFocuses =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      string <- elements ["x", "yz", "hello"]
      selection <- genLineEditSelection
      pure (graph, path, string, selection)
    check (graph, path, string, selection) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let edited = editString path string selection Editor {editorDocument = Document rootId graph, editorFocus = Just (Focus path (testFocusState restingState))}
         in (readAt (documentGraph (editorDocument edited)) rootId path === Just (VString string))
              .&&. (editorFocus edited === Just (Focus path (testFocusState selection)))

propEditIntBuffersInvalidAndCommitsValid :: Property
propEditIntBuffersInvalidAndCommitsValid =
  conjoin
    [ readAt (documentGraph (editorDocument validEdit)) rootId [numberLabel] === Just (VInt 42)
    , editorFocus validEdit === Just (Focus [numberLabel] (testNumberState "42" selection))
    , readAt (documentGraph (editorDocument invalidEdit)) rootId [numberLabel] === Just (VInt 1)
    , editorFocus invalidEdit === Just (Focus [numberLabel] (testNumberState "-" selection))
    ]
  where
    selection = LineEditSelection 2 2 False
    editor = Editor {editorDocument = Document rootId numberGraph, editorFocus = Nothing}
    validEdit = editInt [numberLabel] "42" selection editor
    invalidEdit = editInt [numberLabel] "-" selection editor

propEditFloatBuffersInvalidAndCommitsValid :: Property
propEditFloatBuffersInvalidAndCommitsValid =
  conjoin
    [ readAt (documentGraph (editorDocument validEdit)) rootId [numberLabel] === Just (VFloat 2.5)
    , editorFocus validEdit === Just (Focus [numberLabel] (testNumberState "2.5" selection))
    , readAt (documentGraph (editorDocument invalidEdit)) rootId [numberLabel] === Just (VFloat 1.5)
    , editorFocus invalidEdit === Just (Focus [numberLabel] (testNumberState "nope" selection))
    ]
  where
    selection = LineEditSelection 3 3 False
    editor = Editor {editorDocument = Document rootId floatGraph, editorFocus = Nothing}
    validEdit = editFloat [numberLabel] "2.5" selection editor
    invalidEdit = editFloat [numberLabel] "nope" selection editor

propBlurStringOnlyClearsMatchingPath :: Property
propBlurStringOnlyClearsMatchingPath =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      pure (graph, path)
    check (graph, path) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let editor = Editor {editorDocument = Document rootId graph, editorFocus = Just (Focus path (testFocusState restingState))}
            otherPath = path <> [head labelPool]
         in (editorFocus (blurString path editor) === Nothing)
              .&&. (editorFocus (blurString otherPath editor) === Just (Focus path (testFocusState restingState)))

propDeleteFocusedEdgeDeletesEdge :: Property
propDeleteFocusedEdgeDeletesEdge =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      pure (graph, path)
    check (graph, path) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let focusedEditor = Editor {editorDocument = Document rootId graph, editorFocus = Just (Focus path (testFocusState restingState))}
            unfocusedEditor = Editor {editorDocument = Document rootId graph, editorFocus = Nothing}
            deleted = deleteFocusedEdge focusedEditor
            ignored = deleteFocusedEdge unfocusedEditor
         in (readAt (documentGraph (editorDocument deleted)) rootId path === Nothing)
              .&&. (editorFocus deleted === Nothing)
              .&&. (documentGraph (editorDocument ignored) === graph)
              .&&. (editorFocus ignored === Nothing)

propGraphContextUsesLibraries :: Property
propGraphContextUsesLibraries =
  conjoin
    [ resolvePath libraryContext [toLibraryLabel, valueLabel] === Just (VString "library")
    , resolvePath documentWinsContext [toLibraryLabel, valueLabel] === Just (VString "document")
    , resolvePath documentWinsContext [toLibraryLabel, libraryOnlyLabel] === Nothing
    ]
  where
    root = UUID.fromWords 900 0 0 1
    libraryNode = UUID.fromWords 901 0 0 1
    toLibraryLabel = UUID.fromWords 902 0 0 1
    valueLabel = UUID.fromWords 903 0 0 1
    libraryOnlyLabel = UUID.fromWords 904 0 0 1
    rootGraph =
      Map.fromList
        [ (root, Map.fromList [(toLibraryLabel, VRef libraryNode)])
        ]
    libraryGraph =
      Map.fromList
        [ ( libraryNode
          , Map.fromList
              [ (valueLabel, VString "library")
              , (libraryOnlyLabel, VString "library only")
              ]
          )
        ]
    documentOverrideGraph =
      Map.insert libraryNode (Map.fromList [(valueLabel, VString "document")]) rootGraph
    libraryContext =
      documentContext (Document root rootGraph) [libraryGraph]
    documentWinsContext =
      documentContext (Document root documentOverrideGraph) [libraryGraph]

propListNodeItemFocusesListElement :: Property
propListNodeItemFocusesListElement =
  editorFocus clicked === Just (Focus thirdItemPath defaultFocusState)
  where
    clicked =
      execState
        (handlePointer (PointerDown 205 25) handler)
        Editor {editorDocument = listItemDocument, editorFocus = Nothing}
    handler =
      runTestRender $ do
        measured <- measureHalay (listItemLayout Nothing)
        placeMeasured measured (Rect 0 0 800 600)

propListNodeItemDeleteSplicesList :: Property
propListNodeItemDeleteSplicesList =
  conjoin
    [ resolvePath deletedContext [listLabel, tailLabel, tailLabel] === Just (VRef nilNode)
    , editorFocus deleted === Nothing
    ]
  where
    deleted =
      execState
        (handleDelete handler)
        Editor {editorDocument = listItemDocument, editorFocus = Just (Focus thirdItemPath defaultFocusState)}
    deletedContext =
      documentContext (editorDocument deleted) []
    handler =
      runTestRender $ do
        measured <- measureHalay (listItemLayout (Just (Focus thirdItemPath defaultFocusState)))
        placeMeasured measured (Rect 0 0 800 600)

listItemLayout :: Maybe Focus -> Halay TestRender TestRender (Handler (State Editor))
listItemLayout focus =
  projectDocument
    (focusedProjection (listProjection `over` rawProjection))
    listItemDocument
    modify
    focus

newtype TestRender a = TestRender
  { runTestRender :: a
  }

instance Functor TestRender where
  fmap change (TestRender value) =
    TestRender (change value)

instance Applicative TestRender where
  pure =
    TestRender
  TestRender change <*> TestRender value =
    TestRender (change value)

instance Monad TestRender where
  TestRender value >>= next =
    next value

instance Canvas.Canvas TestRender where
  clearCanvas _viewport = pure ()
  fillRect _rect _color = pure ()
  strokeRect _rect _color _lineWidth = pure ()
  fillText _point _color _text = pure ()
  fillTextMiddle _point _color _text = pure ()
  measureText string =
    pure
      Canvas.TextMetrics
        { Canvas.textWidth = fromIntegral (length string) * 8
        , Canvas.textActualBoundingBoxAscent = 10
        , Canvas.textActualBoundingBoxDescent = 3
        , Canvas.textFontBoundingBoxAscent = 11
        , Canvas.textFontBoundingBoxDescent = 3
        }

genLineEditSelection :: Gen LineEditSelection
genLineEditSelection =
  LineEditSelection <$> chooseInt (0, 8) <*> chooseInt (0, 8) <*> arbitrary

restingState :: LineEditSelection
restingState = LineEditSelection 0 0 False

testFocusState :: LineEditSelection -> FocusState
testFocusState selection =
  defaultFocusState {focusStringSelection = selection}

testNumberState :: String -> LineEditSelection -> FocusState
testNumberState string selection =
  defaultFocusState {focusNumberEdit = Just (NumberEdit string selection)}

sentinel :: Value
sentinel = VString "##sentinel##"

numberLabel :: UUID.UUID
numberLabel = UUID.fromWords 700 0 0 1

numberGraph :: MapGraph
numberGraph =
  Map.fromList [(rootId, Map.fromList [(numberLabel, VInt 1)])]

floatGraph :: MapGraph
floatGraph =
  Map.fromList [(rootId, Map.fromList [(numberLabel, VFloat 1.5)])]

thirdItemPath :: [UUID]
thirdItemPath =
  [listLabel, tailLabel, tailLabel, headLabel]

listItemDocument :: Document
listItemDocument =
  Document rootId listItemGraph

listItemGraph :: MapGraph
listItemGraph =
  Map.fromList
    [ (rootId, Map.fromList [(listLabel, VRef listCell1)])
    , (listCell1, cons (VString "alpha") (VRef listCell2))
    , (listCell2, cons (VInt 2) (VRef listCell3))
    , (listCell3, cons (VRef listItemNode) (VRef nilNode))
    , (listItemNode, Map.fromList [(nameLabel, VString "node")])
    ]
  where
    cons headValue tailValue =
      Map.fromList [(headLabel, headValue), (tailLabel, tailValue)]

listLabel :: UUID
listLabel = UUID.fromWords 800 0 0 1

listCell1 :: UUID
listCell1 = UUID.fromWords 801 0 0 1

listCell2 :: UUID
listCell2 = UUID.fromWords 802 0 0 1

listCell3 :: UUID
listCell3 = UUID.fromWords 803 0 0 1

listItemNode :: UUID
listItemNode = UUID.fromWords 804 0 0 1

genSetEdge :: MapGraph -> Gen (Editor -> Editor)
genSetEdge _graph =
  setEdge <$> genAnyEdge <*> genValue

genDeleteEdge :: MapGraph -> Gen (Editor -> Editor)
genDeleteEdge _graph =
  deleteEdge <$> genAnyEdge

genAnyEdge :: Gen Edge
genAnyEdge =
  Edge <$> elements nodePool <*> elements labelPool

-- Independent resolver (separate from the tools' own path walk) so the
-- properties check two implementations against each other.
readAt :: MapGraph -> UUID -> [UUID] -> Maybe Value
readAt graph node path =
  case path of
    [] -> Nothing
    edgeLabel : rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup edgeLabel edges
      case (rest, value) of
        ([], _) -> Just value
        (_, VRef target) -> readAt graph target rest
        _ -> Nothing

writeAt :: MapGraph -> UUID -> [UUID] -> Value -> Maybe MapGraph
writeAt graph node path newValue =
  case path of
    [] -> Nothing
    edgeLabel : rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup edgeLabel edges
      case (rest, value) of
        ([], VString _) -> Just (Map.insert node (Map.insert edgeLabel newValue edges) graph)
        (_, VRef target) -> writeAt graph target rest newValue
        _ -> Nothing

nodePool :: [UUID]
nodePool = uuidsFrom 1 8

labelPool :: [UUID]
labelPool = uuidsFrom 100 8

rootId :: UUID
rootId = head nodePool

uuidsFrom :: Int -> Int -> [UUID]
uuidsFrom start count =
  [UUID.fromWords (fromIntegral seed) 0 0 1 | seed <- [start .. start + count - 1]]

genGraph :: Gen MapGraph
genGraph = do
  nodes <- sublistOf nodePool `suchThat` elem rootId
  Map.fromList <$> traverse genNode nodes
  where
    genNode node = do
      labels <- sublistOf labelPool `suchThat` (not . null)
      edges <- traverse genEdge labels
      anchor <- genEdge (head labelPool)
      -- fromList is right-biased; the anchor must win so every node keeps
      -- a string edge and the path walk always terminates.
      pure (node, Map.fromList (edges <> [forceString anchor]))
    genEdge edgeLabel = (,) edgeLabel <$> genValue
    forceString (edgeLabel, _) = (edgeLabel, VString "anchor")

genValue :: Gen Value
genValue =
  frequency
    [ (3, VString <$> elements ["a", "bb", "ccc"])
    , (1, VInt <$> arbitrary)
    , (1, VBool <$> arbitrary)
    , (2, VRef <$> elements nodePool)
    ]

-- Random walk to a string spot, bounded against cycles, retried until
-- it lands.
genPath :: MapGraph -> Gen [UUID]
genPath graph =
  walkNode rootId (8 :: Int) `suchThatMap` id
  where
    walkNode node fuel =
      case Map.lookup node graph of
        Nothing -> pure Nothing
        Just edges -> do
          (edgeLabel, value) <- elements (Map.toList edges)
          case value of
            VString _ -> pure (Just [edgeLabel])
            VRef target | fuel > 0 -> fmap (edgeLabel :) <$> walkNode target (fuel - 1)
            _ -> pure Nothing
