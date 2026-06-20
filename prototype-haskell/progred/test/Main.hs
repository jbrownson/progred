module Main
  ( main
  ) where

import qualified Data.Map.Strict as Map
import qualified Data.UUID.Types as UUID
import Control.Monad.Trans.State.Strict (State, execState, modify, put)
import Data.List (find)
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.MapGraph (MapGraph)
import Progred.Projection
import Progred.Render.Graph
import Progred.Render.List
import Progred.Render.Raw
import qualified Puri.Canvas as Canvas
import Puri.Handler
import qualified Puri.KeyCode as KeyCode
import Puri.Widgets (LineEditSelection (..), emptyLineEditSelection, lineEditSelectionAtEnd)
import Test.QuickCheck

main :: IO ()
main = do
  run "setEdge" (propToolTracksValue genSetEdge)
  run "deleteEdge" (propToolTracksValue genDeleteEdge)
  run "editString" propEditStringWritesAndFocuses
  run "editRootString" propEditRootStringWritesDocumentRoot
  run "editInt" propEditIntBuffersInvalidAndCommitsValid
  run "editFloat" propEditFloatBuffersInvalidAndCommitsValid
  run "insertStringEdge" propInsertStringEdgeWritesAndFocuses
  run "blurString" propBlurStringOnlyClearsMatchingPath
  run "deleteFocusedSpotEdge" propDeleteFocusedSpotDeletesEdge
  run "deleteFocusedSpotRoot" propDeleteFocusedSpotClearsDocumentRoot
  run "toggleCollapse" propToggleCollapseTracksPath
  run "graphContext" propGraphContextUsesLibraries
  run "graphSnapshot" propGraphSnapshotIncludesDocumentStructure
  run "graphSnapshotFocus" propGraphSnapshotHighlightsFocusedEdgeAndNode
  run "graphSnapshotSelection" propGraphSnapshotHighlightsGraphSelection
  run "graphPanelSelect" propGraphPanelClickSelectsNode
  run "graphPanelSelectEdge" propGraphPanelClickSelectsEdge
  run "graphPanelClear" propGraphPanelClickClearsSelection
  run "graphLayout" propGraphLayoutTracksSnapshotNodes
  run "graphPanelDrag" propGraphPanelDragStartsAndMovesNode
  run "graphPanelPan" propGraphPanelPanMovesViewport
  run "graphPanelWheelZoom" propGraphPanelWheelZoomsViewport
  run "graphPanelWheelPan" propGraphPanelWheelPansTrackpad
  run "pointerCapture" propPointerCapturePrecedesNormalPointer
  run "listProjectionRequiresIsa" propListProjectionRequiresIsa
  run "listItemFocus" propListNodeItemFocusesListElement
  run "listItemDelete" propListNodeItemDeleteSplicesList
  run "listBeforeFirstInsert" propListNodeInsertBeforeFirstCommitsString
  run "listBeforeFirstRefInsert" propListNodeInsertBeforeFirstCommitsRef
  run "listItemInsert" propListNodeItemInsertCommitsString
  run "rootFocus" propRootNodeFocusesOnClick
  run "rootPlaceholderInsert" propRootPlaceholderCommitsString
  run "commandClickNodeRef" propCommandClickNodeReplacesPendingRawEdgeWithRef
  run "rawCycleProjection" propRawCycleProjectsCollapsedAndExpanded
  run "rawNodeInsertNested" propRawNodeInsertCommitsNestedString
  run "rawNodeInsertSibling" propRawEdgeInsertCommitsSiblingString
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
            let edited = tool (testEditor (testDocument instrumented) (Just (Focus path (testFocusState restingState))))
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
        let edited = editString path string selection (testEditor (testDocument graph) (Just (Focus path (testFocusState restingState))))
         in (readAt (documentGraph (editorDocument edited)) rootId path === Just (VString string))
              .&&. (editorFocus edited === Just (Focus path (testFocusState selection)))

propEditRootStringWritesDocumentRoot :: Property
propEditRootStringWritesDocumentRoot =
  conjoin
    [ documentRoot (editorDocument edited) === Just (VString "root")
    , editorFocus edited === Just (Focus [] (testFocusState selection))
    ]
  where
    selection = lineEditSelectionAtEnd "root"
    edited =
      editString [] "root" selection $
        testEditor (testDocument numberGraph) (Just (Focus [] defaultFocusState))

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
    editor = testEditor (testDocument numberGraph) Nothing
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
    editor = testEditor (testDocument floatGraph) Nothing
    validEdit = editFloat [numberLabel] "2.5" selection editor
    invalidEdit = editFloat [numberLabel] "nope" selection editor

propInsertStringEdgeWritesAndFocuses :: Property
propInsertStringEdgeWritesAndFocuses =
  conjoin
    [ resolvePath editedContext [rawChildLabel, rawInsertedLabel] === Just (VString "delta")
    , editorFocus edited === Just (Focus [rawChildLabel, rawInsertedLabel] (testFocusState deltaSelection))
    ]
  where
    deltaSelection = lineEditSelectionAtEnd "delta"
    edited =
      insertStringEdge [rawChildLabel] rawInsertedLabel "delta" deltaSelection $
        testEditor rawInsertDocument (Just (Focus [rawChildLabel] defaultFocusState))
    editedContext =
      documentContext (editorDocument edited) []

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
        let editor = testEditor (testDocument graph) (Just (Focus path (testFocusState restingState)))
            otherPath = path <> [head labelPool]
         in (editorFocus (blurString path editor) === Nothing)
              .&&. (editorFocus (blurString otherPath editor) === Just (Focus path (testFocusState restingState)))

propDeleteFocusedSpotDeletesEdge :: Property
propDeleteFocusedSpotDeletesEdge =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      pure (graph, path)
    check (graph, path) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let focusedEditor = testEditor (testDocument graph) (Just (Focus path (testFocusState restingState)))
            unfocusedEditor = testEditor (testDocument graph) Nothing
            deleted = deleteFocusedSpot focusedEditor
            ignored = deleteFocusedSpot unfocusedEditor
         in (readAt (documentGraph (editorDocument deleted)) rootId path === Nothing)
              .&&. (editorFocus deleted === Nothing)
              .&&. (documentGraph (editorDocument ignored) === graph)
              .&&. (editorFocus ignored === Nothing)

propDeleteFocusedSpotClearsDocumentRoot :: Property
propDeleteFocusedSpotClearsDocumentRoot =
  conjoin
    [ documentRoot (editorDocument deleted) === Nothing
    , documentGraph (editorDocument deleted) === rawInsertGraph
    , editorFocus deleted === Nothing
    ]
  where
    deleted =
      deleteFocusedSpot
        (testEditor rawInsertDocument (Just (Focus [] defaultFocusState)))

propToggleCollapseTracksPath :: Property
propToggleCollapseTracksPath =
  conjoin
    [ isCollapsed [rawChildLabel] editor === False
    , isCollapsed [rawChildLabel] collapsed === True
    , isCollapsed [rawStringLabel] collapsed === False
    , isCollapsed [rawChildLabel] expanded === False
    , collapseState [rawChildLabel] expanded === Just False
    ]
  where
    editor = newEditor rawInsertDocument
    collapsed = toggleCollapsed [rawChildLabel] editor
    expanded = toggleCollapsed [rawChildLabel] collapsed

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
      documentContext (Document (Just (VRef root)) rootGraph) [libraryGraph]
    documentWinsContext =
      documentContext (Document (Just (VRef root)) documentOverrideGraph) [libraryGraph]

propGraphSnapshotIncludesDocumentStructure :: Property
propGraphSnapshotIncludesDocumentStructure =
  conjoin
    [ (GraphUUID rootId `elem` nodeKeys) === True
    , (GraphUUID rawInsertNode `elem` nodeKeys) === True
    , any ((== Just "\"existing\"") . graphNodeTitle) (graphSnapshotNodes snapshot) === True
    , any isChildEdge (graphSnapshotEdges snapshot) === True
    , any graphNodeRoot (graphSnapshotNodes snapshot) === True
    ]
  where
    snapshot =
      graphSnapshot (newEditor rawInsertDocument) Nothing
    nodeKeys =
      graphNodeKey <$> graphSnapshotNodes snapshot
    isChildEdge edge =
      graphEdgeSource edge == GraphUUID rootId
        && graphEdgeLabel edge == rawChildLabel
        && graphEdgeTarget edge == GraphUUID rawInsertNode

propGraphSnapshotHighlightsFocusedEdgeAndNode :: Property
propGraphSnapshotHighlightsFocusedEdgeAndNode =
  conjoin
    [ graphSnapshotSelectedNode snapshot === Just (GraphSelectedNode (GraphUUID rawInsertNode) GraphSelectionSecondary)
    , graphSnapshotSelectedEdge snapshot === Just (GraphSelectedEdge (GraphUUID rootId) rawChildLabel GraphSelectionSecondary)
    ]
  where
    snapshot =
      graphSnapshot (testEditor rawInsertDocument (Just (Focus [rawChildLabel] defaultFocusState))) Nothing

propGraphSnapshotHighlightsGraphSelection :: Property
propGraphSnapshotHighlightsGraphSelection =
  conjoin
    [ graphSnapshotSelectedNode snapshot === Just (GraphSelectedNode (GraphUUID rawInsertNode) GraphSelectionPrimary)
    , graphSnapshotSelectedEdge edgeSnapshot === Just (GraphSelectedEdge (GraphUUID rootId) rawChildLabel GraphSelectionPrimary)
    ]
  where
    snapshot =
      graphSnapshot (newEditor rawInsertDocument) (Just (GraphSelectNode (GraphUUID rawInsertNode)))
    edgeSnapshot =
      graphSnapshot (newEditor rawInsertDocument) (Just (GraphSelectEdge (GraphUUID rootId) rawChildLabel))

propGraphLayoutTracksSnapshotNodes :: Property
propGraphLayoutTracksSnapshotNodes =
  conjoin
    [ graphLayoutNodeCount stepped === length (graphSnapshotNodes snapshot)
    , graphLayoutNodeCount emptied === 0
    ]
  where
    snapshot =
      graphSnapshot (newEditor rawInsertDocument) Nothing
    stepped =
      stepGraphLayout snapshot emptyGraphLayout
    emptied =
      stepGraphLayout (GraphSnapshot [] [] Nothing Nothing) stepped

propGraphPanelDragStartsAndMovesNode :: Property
propGraphPanelDragStartsAndMovesNode =
  conjoin
    [ graphDragTestDrag afterDown === Just expectedDrag
    , graphDragTestMoved afterMove === Just (Point 10 15)
    , graphDragTestEnded afterUp === True
    ]
  where
    afterDown =
      execState
        (handlePointer (PointerDown 160 120 noModifiers) (graphDragHandler Nothing))
        emptyGraphDragTest
    afterMove =
      execState
        (handlePointer (PointerMove 170 135 noModifiers) (graphDragHandler (graphDragTestDrag afterDown)))
        afterDown
    afterUp =
      execState
        (handlePointer (PointerUp 170 135 noModifiers) (graphDragHandler (graphDragTestDrag afterDown)))
        afterMove
    expectedDrag =
      GraphDrag (GraphUUID rootId) (Point 0 0) (Point 0 0)

propGraphPanelClickSelectsNode :: Property
propGraphPanelClickSelectsNode =
  graphInteractionTestSelection afterUp === Just (GraphSelectNode (GraphUUID rootId))
  where
    afterDown =
      execState
        (handlePointer (PointerDown 160 120 noModifiers) (graphInteractionHandler emptyGraphInteractionTest))
        emptyGraphInteractionTest
    afterUp =
      execState
        (handlePointer (PointerUp 160 120 noModifiers) (graphInteractionHandler afterDown))
        afterDown

propGraphPanelClickSelectsEdge :: Property
propGraphPanelClickSelectsEdge =
  graphInteractionTestSelection afterUp
    === Just (GraphSelectEdge (GraphUUID rootId) rawChildLabel)
  where
    Point {pointX, pointY} = graphEdgeLabelClickPoint
    afterDown =
      execState
        (handlePointer (PointerDown pointX pointY noModifiers) (graphInteractionHandler emptyGraphInteractionTest))
        emptyGraphInteractionTest
    afterUp =
      execState
        (handlePointer (PointerUp pointX pointY noModifiers) (graphInteractionHandler afterDown))
        afterDown

propGraphPanelClickClearsSelection :: Property
propGraphPanelClickClearsSelection =
  graphInteractionTestSelection afterUp === Nothing
  where
    selected =
      emptyGraphInteractionTest
        { graphInteractionTestSelection = Just (GraphSelectNode (GraphUUID rootId))
        }
    afterDown =
      execState
        (handlePointer (PointerDown 10 10 noModifiers) (graphInteractionHandler selected))
        selected
    afterUp =
      execState
        (handlePointer (PointerUp 10 10 noModifiers) (graphInteractionHandler afterDown))
        afterDown

propGraphPanelPanMovesViewport :: Property
propGraphPanelPanMovesViewport =
  conjoin
    [ graphInteractionTestPan afterDown === Just (GraphPan (Point 10 10))
    , graphViewportPan (graphInteractionTestViewport afterMove) === Point 10 15
    , graphInteractionTestPan afterUp === Nothing
    ]
  where
    afterDown =
      execState
        (handlePointer (PointerDown 10 10 noModifiers) (graphInteractionHandler emptyGraphInteractionTest))
        emptyGraphInteractionTest
    afterMove =
      execState
        (handlePointer (PointerMove 20 25 noModifiers) (graphInteractionHandler afterDown))
        afterDown
    afterUp =
      execState
        (handlePointer (PointerUp 20 25 noModifiers) (graphInteractionHandler afterMove))
        afterMove

propGraphPanelWheelZoomsViewport :: Property
propGraphPanelWheelZoomsViewport =
  property $
    graphViewportZoom (graphInteractionTestViewport zoomed)
      > graphViewportZoom (graphInteractionTestViewport emptyGraphInteractionTest)
  where
    zoomed =
      execState
        ( handleWheel
            Wheel
              { wheelX = 160
              , wheelY = 120
              , wheelDeltaX = 0
              , wheelDeltaY = -100
              , wheelDeltaMode = 1
              , wheelModifiers = noModifiers
              }
            (graphInteractionHandler emptyGraphInteractionTest)
        )
        emptyGraphInteractionTest

propGraphPanelWheelPansTrackpad :: Property
propGraphPanelWheelPansTrackpad =
  graphViewportPan (graphInteractionTestViewport panned) === Point (-10.2) (-6.8)
  where
    panned =
      execState
        ( handleWheel
            Wheel
              { wheelX = 160
              , wheelY = 120
              , wheelDeltaX = 12
              , wheelDeltaY = 8
              , wheelDeltaMode = 0
              , wheelModifiers = noModifiers
              }
            (graphInteractionHandler emptyGraphInteractionTest)
        )
        emptyGraphInteractionTest

propPointerCapturePrecedesNormalPointer :: Property
propPointerCapturePrecedesNormalPointer =
  execState (handlePointer (PointerDown 0 0 noModifiers) handler) "" === "capture"
  where
    handler =
      onPointerCapture (\_event -> Just (put "capture"))
        <> onPointer (\_event -> Just (put "normal"))

propListProjectionRequiresIsa :: Property
propListProjectionRequiresIsa =
  case tryProject listProjection env (Cursor [listLabel] Nothing) of
    Nothing -> property True
    Just _ -> counterexample "structural head/tail node without isa projected as a list" False
  where
    env :: Env (State Editor) TestRender
    env =
      Env
        { envContext = documentContext structuralListDocument []
        , envEdit = const (pure ())
        , envFreshUUID = pure listInsertedCell
        , envCollapseState = const Nothing
        , envSecondarySelection = Nothing
        , envProject = const empty
        }

propListNodeItemFocusesListElement :: Property
propListNodeItemFocusesListElement =
  editorFocus clicked === Just (Focus thirdItemPath defaultFocusState)
  where
    clicked =
      execState
        (handlePointer (PointerDown 260 25 noModifiers) handler)
        (testEditor listItemDocument Nothing)
    handler =
      listItemHandler Nothing

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
        (testEditor listItemDocument (Just (Focus thirdItemPath defaultFocusState)))
    deletedContext =
      documentContext (editorDocument deleted) []
    handler =
      listItemHandler (Just (Focus thirdItemPath defaultFocusState))

propListNodeInsertBeforeFirstCommitsString :: Property
propListNodeInsertBeforeFirstCommitsString =
  conjoin
    [ editorFocus pending === Just (Focus beforeFirstItemPendingPath (testPendingState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus beforeFirstItemPendingPath (testPendingState "zero" zeroSelection))
    , resolvePath insertedContext [listLabel] === Just (VRef listInsertedCell)
    , resolvePath insertedContext [listLabel, isaLabel] === Just (VRef listConsNode)
    , resolvePath insertedContext [listLabel, headLabel] === Just (VString "zero")
    , resolvePath insertedContext [listLabel, tailLabel] === Just (VRef listCell1)
    , editorFocus inserted === Just (Focus [listLabel, headLabel] (testFocusState zeroSelection))
    ]
  where
    pending =
      execState
        (handleInsert (listItemHandler (Just (Focus [listLabel] defaultFocusState))))
        (testEditor listItemDocument (Just (Focus [listLabel] defaultFocusState)))
    typed =
      execState
        (handleKey (TextInput "zero") (listItemHandler (editorFocus pending)))
        pending
    inserted =
      execState
        (handleKey enterKey (listItemHandler (editorFocus typed)))
        typed
    insertedContext =
      documentContext (editorDocument inserted) []
    zeroSelection =
      lineEditSelectionAtEnd "zero"

propListNodeInsertBeforeFirstCommitsRef :: Property
propListNodeInsertBeforeFirstCommitsRef =
  conjoin
    [ resolvePath insertedContext [listLabel] === Just (VRef listInsertedCell)
    , resolvePath insertedContext [listLabel, isaLabel] === Just (VRef listConsNode)
    , resolvePath insertedContext [listLabel, headLabel] === Just (VRef listItemNode)
    , resolvePath insertedContext [listLabel, tailLabel] === Just (VRef listCell1)
    , editorFocus inserted === Just (Focus [listLabel, headLabel] defaultFocusState)
    ]
  where
    inserted =
      replaceFocusedSpot listInsertedCell (VRef listItemNode)
        (testEditor listItemDocument (Just (Focus beforeFirstItemPendingPath (testPendingState "" emptyLineEditSelection))))
    insertedContext =
      documentContext (editorDocument inserted) []

propListNodeItemInsertCommitsString :: Property
propListNodeItemInsertCommitsString =
  conjoin
    [ editorFocus pending === Just (Focus afterThirdItemPendingPath (testPendingState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus afterThirdItemPendingPath (testPendingState "omega" omegaSelection))
    , resolvePath insertedContext afterThirdItemPath === Just (VRef listInsertedCell)
    , resolvePath insertedContext (afterThirdItemPath <> [isaLabel]) === Just (VRef listConsNode)
    , resolvePath insertedContext (afterThirdItemPath <> [headLabel]) === Just (VString "omega")
    , resolvePath insertedContext (afterThirdItemPath <> [tailLabel]) === Just (VRef nilNode)
    , editorFocus inserted === Just (Focus (afterThirdItemPath <> [headLabel]) (testFocusState omegaSelection))
    ]
  where
    pending =
      execState
        (handleInsert (listItemHandler (Just (Focus thirdItemPath defaultFocusState))))
        (testEditor listItemDocument (Just (Focus thirdItemPath defaultFocusState)))
    typed =
      execState
        (handleKey (TextInput "omega") (listItemHandler (editorFocus pending)))
        pending
    inserted =
      execState
        (handleKey enterKey (listItemHandler (editorFocus typed)))
        typed
    insertedContext =
      documentContext (editorDocument inserted) []
    omegaSelection =
      lineEditSelectionAtEnd "omega"

propRootNodeFocusesOnClick :: Property
propRootNodeFocusesOnClick =
  editorFocus clicked === Just (Focus [] defaultFocusState)
  where
    clicked =
      execState
        (handlePointer (PointerDown 10 10 noModifiers) (rawInsertHandler Nothing))
        (testEditor rawInsertDocument Nothing)

propRootPlaceholderCommitsString :: Property
propRootPlaceholderCommitsString =
  conjoin
    [ editorFocus focused === Just (Focus [] (testPendingState "" dragSelection))
    , editorFocus typed === Just (Focus [] (testPendingState "root" rootSelection))
    , documentRoot (editorDocument inserted) === Just (VString "root")
    , editorFocus inserted === Just (Focus [] (testFocusState rootSelection))
    ]
  where
    focused =
      execState
        (handlePointer (PointerDown 10 10 noModifiers) (rawDocumentHandler emptyRootDocument Nothing))
        (testEditor emptyRootDocument Nothing)
    typed =
      execState
        (handleKey (TextInput "root") (rawDocumentHandler emptyRootDocument (editorFocus focused)))
        focused
    inserted =
      execState
        (handleKey enterKey (rawDocumentHandler emptyRootDocument (editorFocus typed)))
        typed
    dragSelection =
      LineEditSelection 0 0 True
    rootSelection =
      lineEditSelectionAtEnd "root"

propCommandClickNodeReplacesPendingRawEdgeWithRef :: Property
propCommandClickNodeReplacesPendingRawEdgeWithRef =
  conjoin
    [ resolvePath clickedContext [rawInsertedLabel] === Just (VRef rootId)
    , editorFocus clicked === Just (Focus [rawInsertedLabel] defaultFocusState)
    ]
  where
    clicked =
      execState
        (handlePointer (PointerDown 10 10 commandModifiers) (rawInsertHandler focus))
        (testEditor rawInsertDocument focus)
    focus =
      Just (Focus [rawInsertedLabel] (testPendingState "" emptyLineEditSelection))
    clickedContext =
      documentContext (editorDocument clicked) []

propRawCycleProjectsCollapsedAndExpanded :: Property
propRawCycleProjectsCollapsedAndExpanded =
  conjoin
    [ rawEditorHandler (newEditor cycleDocument) `seq` property True
    , rawEditorHandler (setCollapsed [cycleLabel] False (newEditor cycleDocument)) `seq` property True
    ]

propRawNodeInsertCommitsNestedString :: Property
propRawNodeInsertCommitsNestedString =
  conjoin
    [ editorFocus pending === Just (Focus [rawChildLabel, rawInsertedLabel] (testPendingState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus [rawChildLabel, rawInsertedLabel] (testPendingState "delta" deltaSelection))
    , resolvePath insertedContext [rawChildLabel, rawInsertedLabel] === Just (VString "delta")
    , editorFocus inserted === Just (Focus [rawChildLabel, rawInsertedLabel] (testFocusState deltaSelection))
    ]
  where
    pending =
      execState
        (handleInsert (rawInsertHandler (Just (Focus [rawChildLabel] defaultFocusState))))
        (testEditor rawInsertDocument (Just (Focus [rawChildLabel] defaultFocusState)))
    typed =
      execState
        (handleKey (TextInput "delta") (rawInsertHandler (editorFocus pending)))
        pending
    inserted =
      execState
        (handleKey enterKey (rawInsertHandler (editorFocus typed)))
        typed
    insertedContext =
      documentContext (editorDocument inserted) []
    deltaSelection =
      lineEditSelectionAtEnd "delta"

propRawEdgeInsertCommitsSiblingString :: Property
propRawEdgeInsertCommitsSiblingString =
  conjoin
    [ editorFocus pending === Just (Focus [rawInsertedLabel] (testPendingState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus [rawInsertedLabel] (testPendingState "epsilon" epsilonSelection))
    , resolvePath insertedContext [rawInsertedLabel] === Just (VString "epsilon")
    , editorFocus inserted === Just (Focus [rawInsertedLabel] (testFocusState epsilonSelection))
    ]
  where
    pending =
      execState
        (handleInsert (rawInsertHandler (Just (Focus [rawStringLabel] defaultFocusState))))
        (testEditor rawInsertDocument (Just (Focus [rawStringLabel] defaultFocusState)))
    typed =
      execState
        (handleKey (TextInput "epsilon") (rawInsertHandler (editorFocus pending)))
        pending
    inserted =
      execState
        (handleKey enterKey (rawInsertHandler (editorFocus typed)))
        typed
    insertedContext =
      documentContext (editorDocument inserted) []
    epsilonSelection =
      lineEditSelectionAtEnd "epsilon"

listItemHandler :: Maybe Focus -> Handler (State Editor)
listItemHandler focus =
  runTestRender $ do
    measured <- measureHalay (listItemLayout focus)
    placeMeasured measured (Rect 0 0 800 600)

listItemLayout :: Maybe Focus -> Halay TestRender TestRender (Handler (State Editor))
listItemLayout focus =
  projectDocument
    (focusedProjection (listProjection `over` rawProjection))
    listItemDocument
    modify
    (pure listInsertedCell)
    focus

rawInsertHandler :: Maybe Focus -> Handler (State Editor)
rawInsertHandler focus =
  rawDocumentHandler rawInsertDocument focus

rawDocumentHandler :: Document -> Maybe Focus -> Handler (State Editor)
rawDocumentHandler document focus =
  runTestRender $ do
    measured <- measureHalay (rawDocumentLayout document focus)
    placeMeasured measured (Rect 0 0 800 600)

rawEditorHandler :: Editor -> Handler (State Editor)
rawEditorHandler editor =
  runTestRender $ do
    measured <- measureHalay (rawEditorLayout editor)
    placeMeasured measured (Rect 0 0 800 600)

rawDocumentLayout :: Document -> Maybe Focus -> Halay TestRender TestRender (Handler (State Editor))
rawDocumentLayout document focus =
  projectDocument
    (focusedProjection rawProjection)
    document
    modify
    (pure rawInsertedLabel)
    focus

rawEditorLayout :: Editor -> Halay TestRender TestRender (Handler (State Editor))
rawEditorLayout editor =
  projectEditor
    (focusedProjection rawProjection)
    editor
    modify
    (pure rawInsertedLabel)
    Nothing

data GraphDragTest = GraphDragTest
  { graphDragTestDrag :: Maybe GraphDrag
  , graphDragTestMoved :: Maybe Point
  , graphDragTestEnded :: Bool
  , graphDragTestSelection :: Maybe GraphSelection
  }
  deriving (Eq, Show)

emptyGraphDragTest :: GraphDragTest
emptyGraphDragTest =
  GraphDragTest Nothing Nothing False Nothing

data GraphInteractionTest = GraphInteractionTest
  { graphInteractionTestDrag :: Maybe GraphDrag
  , graphInteractionTestMoved :: Maybe Point
  , graphInteractionTestEnded :: Bool
  , graphInteractionTestPan :: Maybe GraphPan
  , graphInteractionTestEdgePress :: Maybe GraphEdge
  , graphInteractionTestViewport :: GraphViewport
  , graphInteractionTestPointerOrigin :: Maybe Point
  , graphInteractionTestPointerMoved :: Bool
  , graphInteractionTestSelection :: Maybe GraphSelection
  }
  deriving (Eq, Show)

emptyGraphInteractionTest :: GraphInteractionTest
emptyGraphInteractionTest =
  GraphInteractionTest Nothing Nothing False Nothing Nothing emptyGraphViewport Nothing False Nothing

graphEdgeLabelClickPoint :: Point
graphEdgeLabelClickPoint =
  runTestRender $ do
    let snapshot = graphSnapshot (newEditor rawInsertDocument) Nothing
        layout = stepGraphLayout snapshot emptyGraphLayout
    nodeSizes <- traverse nodeSize (graphSnapshotNodes snapshot)
    hits <-
      graphEdgeLabelHitAreas snapshot emptyGraphViewport layout (Rect 0 0 320 240) nodeSizes
    case
      find
        ( \GraphEdgeLabelHit {graphEdgeLabelHitEdge = edge} ->
            graphEdgeSource edge == GraphUUID rootId && graphEdgeLabel edge == rawChildLabel
        )
        hits
      of
      Just GraphEdgeLabelHit {graphEdgeLabelHitRect = rect} ->
        pure (Point (x rect + width rect / 2) (y rect + height rect / 2))
      Nothing -> error "expected child edge label hit area"

graphDragHandler :: Maybe GraphDrag -> Handler (State GraphDragTest)
graphDragHandler drag =
  runTestRender $ do
    measured <-
      measureHalay $
        graphPanel
          (graphSnapshot (newEditor rawInsertDocument) Nothing)
          emptyGraphViewport
          emptyGraphLayout
          GraphPanelActions
            { graphPanelDrag = drag
            , graphPanelPan = Nothing
            , graphPanelEdgePress = Nothing
            , graphPanelViewport = emptyGraphViewport
            , graphPanelPointerOrigin = Nothing
            , graphPanelPointerMoved = False
            , graphPanelDragStart = \newDrag -> modify (\state -> state {graphDragTestDrag = Just newDrag})
            , graphPanelDragMove = \position -> modify (\state -> state {graphDragTestMoved = Just position})
            , graphPanelDragEnd = modify (\state -> state {graphDragTestEnded = True})
            , graphPanelPanStart = \_ -> pure ()
            , graphPanelPanMove = \_ -> pure ()
            , graphPanelPanEnd = pure ()
            , graphPanelEdgePressStart = \_ -> pure ()
            , graphPanelEdgePressEnd = pure ()
            , graphPanelSetViewport = \_ -> pure ()
            , graphPanelInteractionStart = \_ -> pure ()
            , graphPanelInteractionMove = \_ -> pure ()
            , graphPanelSetSelection = \selection -> modify (\state -> state {graphDragTestSelection = selection})
            }
    placeMeasured measured (Rect 0 0 320 240)

graphInteractionHandler :: GraphInteractionTest -> Handler (State GraphInteractionTest)
graphInteractionHandler state =
  runTestRender $ do
    measured <-
      measureHalay $
        graphPanel
          (graphSnapshot (newEditor rawInsertDocument) Nothing)
          (graphInteractionTestViewport state)
          emptyGraphLayout
          GraphPanelActions
            { graphPanelDrag = graphInteractionTestDrag state
            , graphPanelPan = graphInteractionTestPan state
            , graphPanelEdgePress = graphInteractionTestEdgePress state
            , graphPanelViewport = graphInteractionTestViewport state
            , graphPanelPointerOrigin = graphInteractionTestPointerOrigin state
            , graphPanelPointerMoved = graphInteractionTestPointerMoved state
            , graphPanelDragStart = \newDrag -> modify (\current -> current {graphInteractionTestDrag = Just newDrag})
            , graphPanelDragMove = \position -> modify (\current -> current {graphInteractionTestMoved = Just position})
            , graphPanelDragEnd = modify (\current -> current {graphInteractionTestEnded = True, graphInteractionTestDrag = Nothing})
            , graphPanelPanStart = \pointer -> modify (\current -> current {graphInteractionTestPan = Just (GraphPan pointer)})
            , graphPanelPanMove = \pointer ->
                modify
                  ( \current ->
                      case graphInteractionTestPan current of
                        Just pan ->
                          let (viewport, panState) =
                                moveGraphPan pointer (graphInteractionTestViewport current) pan
                           in current {graphInteractionTestViewport = viewport, graphInteractionTestPan = Just panState}
                        Nothing -> current
                  )
            , graphPanelPanEnd = modify (\current -> current {graphInteractionTestPan = Nothing})
            , graphPanelEdgePressStart = \edge -> modify (\current -> current {graphInteractionTestEdgePress = Just edge})
            , graphPanelEdgePressEnd = modify (\current -> current {graphInteractionTestEdgePress = Nothing})
            , graphPanelSetViewport = \viewport -> modify (\current -> current {graphInteractionTestViewport = viewport})
            , graphPanelInteractionStart = \pointer ->
                modify (\current -> current {graphInteractionTestPointerOrigin = Just pointer, graphInteractionTestPointerMoved = False})
            , graphPanelInteractionMove = \pointer ->
                modify
                  ( \current ->
                      case (graphInteractionTestPointerOrigin current, graphInteractionTestPointerMoved current) of
                        (Just origin, False)
                          | graphPointerExceededClickThreshold origin pointer ->
                              current {graphInteractionTestPointerMoved = True}
                        _ -> current
                  )
            , graphPanelSetSelection = \selection -> modify (\current -> current {graphInteractionTestSelection = selection})
            }
    placeMeasured measured (Rect 0 0 320 240)

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
  strokeLine _start _end _color _lineWidth = pure ()
  fillText _point _color _text = pure ()
  fillTextMiddle _point _color _text = pure ()
  withClip _rect action = action
  withGraphTransform _origin _zoom action = action
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

testEditor :: Document -> Maybe Focus -> Editor
testEditor document focus =
  (newEditor document) {editorFocus = focus}

testFocusState :: LineEditSelection -> FocusState
testFocusState selection =
  defaultFocusState {focusStringSelection = selection}

testNumberState :: String -> LineEditSelection -> FocusState
testNumberState string selection =
  defaultFocusState {focusNumberEdit = Just (NumberEdit string selection)}

testPendingState :: String -> LineEditSelection -> FocusState
testPendingState string selection =
  defaultFocusState {focusPendingEdit = Just (PendingEdit string selection)}

enterKey :: KeyEvent
enterKey =
  KeyCode noModifiers KeyCode.enter

noModifiers :: KeyModifiers
noModifiers =
  KeyModifiers
    { keyShift = False
    , keyAlt = False
    , keyCtrl = False
    , keyMeta = False
    }

commandModifiers :: KeyModifiers
commandModifiers =
  noModifiers {keyMeta = True}

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

afterThirdItemPath :: [UUID]
afterThirdItemPath =
  [listLabel, tailLabel, tailLabel, tailLabel]

beforeFirstItemPendingPath :: [UUID]
beforeFirstItemPendingPath =
  [listLabel, listBeforeLabel]

afterThirdItemPendingPath :: [UUID]
afterThirdItemPendingPath =
  afterThirdItemPath <> [listBeforeLabel]

listItemDocument :: Document
listItemDocument =
  testDocument listItemGraph

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
      Map.fromList [(isaLabel, VRef listConsNode), (headLabel, headValue), (tailLabel, tailValue)]

structuralListDocument :: Document
structuralListDocument =
  testDocument structuralListGraph

structuralListGraph :: MapGraph
structuralListGraph =
  Map.fromList
    [ (rootId, Map.fromList [(listLabel, VRef structuralListCell)])
    , (structuralListCell, Map.fromList [(headLabel, VString "alpha"), (tailLabel, VRef nilNode)])
    ]

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

listInsertedCell :: UUID
listInsertedCell = UUID.fromWords 805 0 0 1

structuralListCell :: UUID
structuralListCell = UUID.fromWords 806 0 0 1

rawInsertDocument :: Document
rawInsertDocument =
  testDocument rawInsertGraph

emptyRootDocument :: Document
emptyRootDocument =
  Document Nothing rawInsertGraph

testDocument :: MapGraph -> Document
testDocument =
  Document (Just (VRef rootId))

rawInsertGraph :: MapGraph
rawInsertGraph =
  Map.fromList
    [ ( rootId
      , Map.fromList
          [ (rawChildLabel, VRef rawInsertNode)
          , (rawStringLabel, VString "existing")
          ]
      )
    , (rawInsertNode, Map.fromList [(nameLabel, VString "child")])
    ]

rawChildLabel :: UUID
rawChildLabel = UUID.fromWords 810 0 0 1

rawStringLabel :: UUID
rawStringLabel = UUID.fromWords 811 0 0 1

rawInsertNode :: UUID
rawInsertNode = UUID.fromWords 812 0 0 1

rawInsertedLabel :: UUID
rawInsertedLabel = UUID.fromWords 813 0 0 1

cycleDocument :: Document
cycleDocument =
  testDocument cycleGraph

cycleGraph :: MapGraph
cycleGraph =
  Map.fromList
    [ ( rootId
      , Map.fromList
          [ (nameLabel, VString "cycle")
          , (cycleLabel, VRef rootId)
          ]
      )
    ]

cycleLabel :: UUID
cycleLabel = UUID.fromWords 814 0 0 1

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
