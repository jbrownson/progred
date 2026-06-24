module Main
  ( main
  ) where

import qualified Data.Map.Strict as Map
import qualified Data.UUID.Types as UUID
import Control.Monad.Trans.State.Strict (State, execState, gets, modify, put)
import Data.List (find)
import Data.Maybe (isJust)
import Halay
import Progred.App
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
import Puri.Widgets.ScrollViewport
  ( ScrollPlacementTrace (..)
  , clampScrollOffset
  , scrollViewport
  , traceScrollPlacement
  )
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
  run "deleteGraphSelectionEdge" propDeleteGraphSelectionDeletesEdge
  run "deleteGraphSelectionNode" propDeleteGraphSelectionDeletesNodeEdges
  run "deleteGraphSelectionRoot" propDeleteGraphSelectionClearsDocumentRoot
  run "deleteGraphSelectionScalar" propDeleteGraphSelectionDeletesScalarEdges
  run "deleteGraphSelectionNoOp" propDeleteGraphSelectionIgnoresMissingEdge
  run "deleteActiveGraphSelection" propDeleteActiveGraphSelectionClearsSelection
  run "toggleCollapse" propToggleCollapseTracksPath
  run "graphContext" propGraphContextUsesLibraries
  run "graphSnapshot" propGraphSnapshotIncludesDocumentStructure
  run "graphSnapshotDedup" propGraphSnapshotMergesDuplicateValues
  run "graphSnapshotDedupEdit" propGraphSnapshotDedupWhileEditingMatchingString
  run "graphSnapshotOrphans" propGraphSnapshotIncludesOrphanNodes
  run "graphSnapshotFocus" propGraphSnapshotHighlightsFocusedEdgeAndNode
  run "graphSnapshotSelection" propGraphSnapshotHighlightsGraphSelection
  run "graphPanelSelect" propGraphPanelClickSelectsNode
  run "graphPanelSelectEdge" propGraphPanelClickSelectsEdge
  run "graphPanelClear" propGraphPanelClickClearsSelection
  run "graphLayout" propGraphLayoutTracksSnapshotNodes
  run "graphLayoutEdit" propGraphLayoutStableWhileEditingString
  run "graphLayoutBlur" propGraphLayoutContinuesAfterBlurString
  run "graphPanelDrag" propGraphPanelDragStartsAndMovesNode
  run "graphPanelPan" propGraphPanelPanMovesViewport
  run "graphPanelWheelZoom" propGraphPanelWheelZoomsViewport
  run "graphPanelWheelPan" propGraphPanelWheelPansTrackpad
  run "treeSecondaryEdge" propTreeSecondaryHighlightEdge
  run "treeSecondaryString" propTreeSecondaryHighlightString
  run "treeSecondarySharedString" propTreeSecondaryHighlightSharedString
  run "treeSecondaryUnderSelection" propTreeSecondaryHighlightSuppressesUnderSelection
  run "clearActiveSelection" propClearActiveSelectionClearsGraph
  run "treeFocusSelection" propTreeFocusReplacesGraphSelection
  run "pointerCapture" propPointerCapturePrecedesNormalPointer
  run "listProjectionRequiresIsa" propListProjectionRequiresIsa
  run "listItemFocus" propListNodeItemFocusesListElement
  run "listItemDelete" propListNodeItemDeleteSplicesList
  run "listBeforeFirstInsert" propListNodeInsertBeforeFirstCommitsString
  run "listBeforeFirstRefInsert" propListNodeInsertBeforeFirstCommitsRef
  run "listItemInsert" propListNodeItemInsertCommitsString
  run "listItemEnterCompose" propListNodeItemEnterStartsEdgeCompose
  run "rootFocus" propRootNodeFocusesOnClick
  run "rootPlaceholderInsert" propRootPlaceholderCommitsString
  run "commandClickNodeRef" propCommandClickNodeReplacesPendingRawEdgeWithRef
  run "commandClickEdgeLabel" propCommandClickLabelChoosesEdgeComposeLabel
  run "chooseExistingEdgeLabel" propChooseExistingEdgeLabelFocusesSpot
  run "commandClickNodeLabel" propCommandClickNodeChoosesEdgeComposeLabel
  run "graphComposePickNode" propGraphComposePickSelectsNodeRef
  run "rawCycleProjection" propRawCycleProjectsCollapsedAndExpanded
  run "rawNodeInsertNested" propRawNodeInsertCommitsNestedString
  run "rawNodeInsertSibling" propRawEdgeInsertCommitsSiblingString
  run "scrollClamp" propClampScrollOffset
  run "scrollWheelInside" propScrollViewportWheelInsideViewport
  run "scrollWheelTrackpad" propScrollViewportTrackpadScrollDirection
  run "scrollWheelAccumulates" propScrollViewportWheelAccumulates
  run "scrollWheelClamp" propScrollViewportWheelClampsToContent
  run "scrollChildPlacementY" propScrollChildPlacementYMoves
  run "scrollClipViewportClamp" propScrollClipViewportClampsOffset
  run "scrollClampUsesClipNotLayoutRect" propScrollClampUsesClipNotLayoutRect
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

propDeleteGraphSelectionDeletesEdge :: Property
propDeleteGraphSelectionDeletesEdge =
  conjoin
    [ readAt (documentGraph (editorDocument deleted)) rootId [rawChildLabel] === Nothing
    , readAt (documentGraph (editorDocument deleted)) rootId [rawStringLabel] === Just (VString "existing")
    , documentRoot (editorDocument deleted) === Just (VRef rootId)
    ]
  where
    deleted =
      deleteGraphSelection
        (GraphSelectEdge (GraphUUID rootId) rawChildLabel)
        (newEditor rawInsertDocument)

propDeleteGraphSelectionDeletesNodeEdges :: Property
propDeleteGraphSelectionDeletesNodeEdges =
  conjoin
    [ readAt (documentGraph (editorDocument deleted)) rootId [rawChildLabel] === Nothing
    , readAt (documentGraph (editorDocument deleted)) rootId [rawStringLabel] === Just (VString "existing")
    , Map.lookup rawInsertNode (documentGraph (editorDocument deleted)) === Just Map.empty
    , documentRoot (editorDocument deleted) === Just (VRef rootId)
    ]
  where
    deleted =
      deleteGraphSelection
        (GraphSelectNode (GraphUUID rawInsertNode))
        (newEditor rawInsertDocument)

propDeleteGraphSelectionClearsDocumentRoot :: Property
propDeleteGraphSelectionClearsDocumentRoot =
  conjoin
    [ documentRoot (editorDocument deleted) === Nothing
    , Map.lookup rootId (documentGraph (editorDocument deleted)) === Just Map.empty
    , Map.lookup rawInsertNode (documentGraph (editorDocument deleted))
        === Just (Map.fromList [(nameLabel, VString "child")])
    ]
  where
    deleted =
      deleteGraphSelection
        (GraphSelectNode (GraphUUID rootId))
        (newEditor rawInsertDocument)

propDeleteGraphSelectionDeletesScalarEdges :: Property
propDeleteGraphSelectionDeletesScalarEdges =
  conjoin
    [ readAt (documentGraph (editorDocument deleted)) rootId [rawStringLabel] === Nothing
    , readAt (documentGraph (editorDocument deleted)) rootId [sharedStringLabel] === Nothing
    , documentRoot (editorDocument deleted) === Just (VRef rootId)
    ]
  where
    deleted =
      deleteGraphSelection
        (GraphSelectNode (GraphScalar (StringKey "shared")))
        (newEditor sharedStringDocument)

propDeleteGraphSelectionIgnoresMissingEdge :: Property
propDeleteGraphSelectionIgnoresMissingEdge =
  documentGraph (editorDocument deleted) === rawInsertGraph
  where
    deleted =
      deleteGraphSelection
        (GraphSelectEdge (GraphUUID rootId) rawInsertedLabel)
        (newEditor rawInsertDocument)

propDeleteActiveGraphSelectionClearsSelection :: Property
propDeleteActiveGraphSelectionClearsSelection =
  ioProperty $ do
    let graphSelected =
          initialModel
            { modelActiveSelection = ActiveGraph (GraphSelectEdge (GraphUUID rootId) rawChildLabel)
            , modelEditor = newEditor rawInsertDocument
            }
    (_, afterDelete) <- runAppM deleteActiveSelection graphSelected
    return $
      conjoin
        [ modelActiveSelection afterDelete === ActiveNone
        , readAt (documentGraph (editorDocument (modelEditor afterDelete))) rootId [rawChildLabel] === Nothing
        , editorFocus (modelEditor afterDelete) === Nothing
        ]

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

sharedStringLabel :: UUID
sharedStringLabel = UUID.fromWords 815 0 0 1

sharedStringDocument :: Document
sharedStringDocument =
  testDocument sharedStringGraph

sharedStringGraph :: MapGraph
sharedStringGraph =
  Map.fromList
    [ ( rootId
      , Map.fromList
          [ (rawStringLabel, VString "shared")
          , (sharedStringLabel, VString "shared")
          ]
      )
    ]

propGraphSnapshotMergesDuplicateValues :: Property
propGraphSnapshotMergesDuplicateValues =
  conjoin
    [ length scalarNodes === 1
    , length refNodes === 1
    , length sharedRefEdges === 2
    ]
  where
    sharedSnapshot =
      graphSnapshot (newEditor sharedStringDocument) Nothing
    scalarNodes =
      [ node
      | node <- graphSnapshotNodes sharedSnapshot
      , case graphNodeKey node of
          GraphScalar _ -> True
          _ -> False
      ]
    refDocument =
      testDocument
        ( Map.insert rootId (Map.fromList [(rawChildLabel, VRef rawInsertNode), (sharedStringLabel, VRef rawInsertNode)]) rawInsertGraph
        )
    refSnapshot =
      graphSnapshot (newEditor refDocument) Nothing
    refNodes =
      [ node
      | node <- graphSnapshotNodes refSnapshot
      , graphNodeKey node == GraphUUID rawInsertNode
      ]
    sharedRefEdges =
      [ edge
      | edge <- graphSnapshotEdges refSnapshot
      , graphEdgeTarget edge == GraphUUID rawInsertNode
      ]

orphanNode :: UUID
orphanNode = UUID.fromWords 816 0 0 1

orphanDocument :: Document
orphanDocument =
  testDocument orphanGraph

orphanGraph :: MapGraph
orphanGraph =
  Map.insert orphanNode (Map.fromList [(nameLabel, VString "orphan")]) rawInsertGraph

propGraphSnapshotIncludesOrphanNodes :: Property
propGraphSnapshotIncludesOrphanNodes =
  (GraphUUID orphanNode `elem` nodeKeys) === True
  where
    snapshot =
      graphSnapshot (newEditor orphanDocument) Nothing
    nodeKeys =
      graphNodeKey <$> graphSnapshotNodes snapshot

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

stringEditingGraph :: String -> MapGraph
stringEditingGraph string =
  Map.adjust (Map.insert rawStringLabel (VString string)) rootId rawInsertGraph

stringEditingEditor :: String -> Editor
stringEditingEditor string =
  testEditor
    (testDocument (stringEditingGraph string))
    (Just (Focus [rawStringLabel] defaultFocusState))

stringEdgeTarget :: GraphSnapshot -> GraphNodeKey
stringEdgeTarget snapshot =
  graphEdgeTarget $
    head
      [ edge
      | edge <- graphSnapshotEdges snapshot
      , graphEdgeLabel edge == rawStringLabel
      ]

settleLayout :: GraphSnapshot -> GraphLayout -> GraphLayout
settleLayout snapshot =
  last . take 24 . iterate (stepGraphLayout snapshot)

pointClose :: Maybe Point -> Maybe Point -> Bool
pointClose (Just (Point x0 y0)) (Just (Point x1 y1)) =
  let dx = x1 - x0
      dy = y1 - y0
   in dx * dx + dy * dy < 150 * 150
pointClose _ _ = False

propGraphLayoutStableWhileEditingString :: Property
propGraphLayoutStableWhileEditingString =
  conjoin
    [ isJust initialPosition === True
    , isJust steppedPosition === True
    , pointClose initialPosition steppedPosition === True
    ]
  where
    strings = ["e", "ex"]
    snapshots =
      [ graphSnapshot (stringEditingEditor string) Nothing
      | string <- strings
      ]
    initialLayout =
      stepGraphLayout (head snapshots) emptyGraphLayout
    steppedLayout =
      stepGraphLayout (last snapshots) initialLayout
    initialPosition =
      graphLayoutPosition (stringEdgeTarget (head snapshots)) initialLayout
    steppedPosition =
      graphLayoutPosition (stringEdgeTarget (last snapshots)) steppedLayout

propGraphSnapshotDedupWhileEditingMatchingString :: Property
propGraphSnapshotDedupWhileEditingMatchingString =
  length scalarNodes === 1
  where
    snapshot =
      graphSnapshot
        ( testEditor
            (testDocument sharedStringGraph)
            (Just (Focus [rawStringLabel] defaultFocusState))
        )
        Nothing
    scalarNodes =
      [ node
      | node <- graphSnapshotNodes snapshot
      , case graphNodeKey node of
          GraphScalar _ -> True
          _ -> False
      ]

propGraphLayoutContinuesAfterBlurString :: Property
propGraphLayoutContinuesAfterBlurString =
  pointClose focusedPositionBeforeBlur blurredPosition === True
  where
    graph =
      stringEditingGraph "existing"
    focusedSnapshot =
      graphSnapshot
        (testEditor (testDocument graph) (Just (Focus [rawStringLabel] defaultFocusState)))
        Nothing
    blurredSnapshot =
      graphSnapshot (testEditor (testDocument graph) Nothing) Nothing
    layoutWhileFocused =
      settleLayout focusedSnapshot (stepGraphLayout focusedSnapshot emptyGraphLayout)
    focusedKey =
      stringEdgeTarget focusedSnapshot
    focusedPositionBeforeBlur =
      graphLayoutPosition focusedKey layoutWhileFocused
    layoutAfterBlur =
      stepGraphLayout blurredSnapshot layoutWhileFocused
    blurredKey =
      stringEdgeTarget blurredSnapshot
    blurredPosition =
      graphLayoutPosition blurredKey layoutAfterBlur

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

propTreeSecondaryHighlightEdge :: Property
propTreeSecondaryHighlightEdge =
  treeSecondaryHighlight (newEditor rawInsertDocument) (Just (GraphSelectEdge (GraphUUID rootId) rawChildLabel))
    === Just (SecondarySpot [rawChildLabel])

propTreeSecondaryHighlightString :: Property
propTreeSecondaryHighlightString =
  treeSecondaryHighlight (newEditor rawInsertDocument) (Just (GraphSelectNode (GraphScalar (StringKey "existing"))))
    === Just (SecondaryScalar (StringKey "existing"))

propTreeSecondaryHighlightSharedString :: Property
propTreeSecondaryHighlightSharedString =
  treeSecondaryHighlight (newEditor sharedStringDocument) (Just (GraphSelectNode (GraphScalar (StringKey "shared"))))
    === Just (SecondaryScalar (StringKey "shared"))

propTreeSecondaryHighlightSuppressesUnderSelection :: Property
propTreeSecondaryHighlightSuppressesUnderSelection =
  conjoin
    [ treeSecondaryHighlight labelCompose Nothing === Nothing
    , treeSecondaryHighlight valueCompose Nothing === Nothing
    , treeSecondaryHighlight labelCompose (Just (GraphSelectNode (GraphUUID rawInsertNode))) === Nothing
    ]
  where
    labelCompose =
      testEditor rawInsertDocument (Just (Focus [] (testPendingLabelState "" emptyLineEditSelection)))
    valueCompose =
      testEditor rawInsertDocument (Just (Focus [rawChildLabel] (testPendingValueState "" emptyLineEditSelection)))

propClearActiveSelectionClearsGraph :: Property
propClearActiveSelectionClearsGraph =
  ioProperty $ do
    let graphSelected =
          initialModel
            { modelActiveSelection = ActiveGraph (GraphSelectNode (GraphUUID rawInsertNode))
            , modelEditor =
                setFocus (Just (Focus [rawChildLabel] defaultFocusState)) (newEditor rawInsertDocument)
            }
    (_, cleared) <- runAppM clearActiveSelection graphSelected
    return $
      conjoin
        [ modelActiveSelection cleared === ActiveNone
        , editorFocus (modelEditor cleared) === Nothing
        ]

propTreeFocusReplacesGraphSelection :: Property
propTreeFocusReplacesGraphSelection =
  activeSelectionAfterEdit
    (newEditor rawInsertDocument)
    focused
    (ActiveGraph (GraphSelectNode (GraphUUID rootId)))
    === ActiveDocument (Focus [rawChildLabel] defaultFocusState)
  where
    focused =
      setFocus (Just (Focus [rawChildLabel] defaultFocusState)) (newEditor rawInsertDocument)

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
        , envFocus = Nothing
        , envSecondaryHighlight = Nothing
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
        (handleKey commaKey (listItemHandler (Just (Focus [listLabel] defaultFocusState))))
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
        (handleKey commaKey (listItemHandler (Just (Focus thirdItemPath defaultFocusState))))
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

propListNodeItemEnterStartsEdgeCompose :: Property
propListNodeItemEnterStartsEdgeCompose =
  editorFocus pending === Just (Focus thirdItemPath (testPendingLabelState "" emptyLineEditSelection))
  where
    pending =
      execState
        (handleInsert (listItemHandler (Just (Focus thirdItemPath defaultFocusState))))
        (testEditor listItemDocument (Just (Focus thirdItemPath defaultFocusState)))

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
      Just (Focus [rawInsertedLabel] (testPendingValueState "" emptyLineEditSelection))
    clickedContext =
      documentContext (editorDocument clicked) []

graphPanelNoCompose :: Document -> GraphPanelActions (State a)
graphPanelNoCompose document =
  GraphPanelActions
    { graphPanelDrag = Nothing
    , graphPanelPan = Nothing
    , graphPanelEdgePress = Nothing
    , graphPanelViewport = emptyGraphViewport
    , graphPanelPointerOrigin = Nothing
    , graphPanelPointerMoved = False
    , graphPanelDocument = document
    , graphPanelComposeMode = Nothing
    , graphPanelComposePickLabel = const (pure ())
    , graphPanelComposePickValue = const (pure ())
    , graphPanelDragStart = const (pure ())
    , graphPanelDragMove = const (pure ())
    , graphPanelDragEnd = pure ()
    , graphPanelPanStart = const (pure ())
    , graphPanelPanMove = const (pure ())
    , graphPanelPanEnd = pure ()
    , graphPanelEdgePressStart = const (pure ())
    , graphPanelEdgePressEnd = pure ()
    , graphPanelSetViewport = const (pure ())
    , graphPanelInteractionStart = const (pure ())
    , graphPanelInteractionMove = const (pure ())
    , graphPanelSetSelection = const (pure ())
    }

propCommandClickNodeChoosesEdgeComposeLabel :: Property
propCommandClickNodeChoosesEdgeComposeLabel =
  property $
    case find picksLabel clickGrid of
      Just point ->
        editorFocus (clickLabel point)
          === Just (Focus [rawInsertNode] (testPendingValueState "" emptyLineEditSelection))
      Nothing ->
        counterexample "no coordinate cmd+clicked a node identicon" False
  where
    pending =
      testEditor rawInsertDocument (Just (Focus [] (testPendingLabelState "" emptyLineEditSelection)))
    clickLabel (x, y) =
      execState
        (handlePointer (PointerDown x y commandModifiers) (rawInsertHandler (editorFocus pending)))
        pending
    picksLabel point =
      editorFocus (clickLabel point)
        == Just (Focus [rawInsertNode] (testPendingValueState "" emptyLineEditSelection))
    clickGrid =
      [(x, y) | x <- [0, 2 .. 240], y <- [0, 2 .. 240]]

propGraphComposePickSelectsNodeRef :: Property
propGraphComposePickSelectsNodeRef =
  conjoin
    [ resolvePath pickedContext [rawInsertedLabel] === Just (VRef rootId)
    , editorFocus picked === Just (Focus [rawInsertedLabel] defaultFocusState)
    ]
  where
    focus =
      Just (Focus [rawInsertedLabel] (testPendingValueState "" emptyLineEditSelection))
    picked =
      execState
        (handlePointer (PointerDown 160 120 commandModifiers) (graphComposeHandler focus))
        (testEditor rawInsertDocument focus)
    pickedContext =
      documentContext (editorDocument picked) []

propChooseExistingEdgeLabelFocusesSpot :: Property
propChooseExistingEdgeLabelFocusesSpot =
  editorFocus focused === Just (Focus [rawChildLabel] defaultFocusState)
  where
    focused =
      chooseEdgeComposeLabel [] rawChildLabel
        (testEditor rawInsertDocument (Just (Focus [] (testPendingLabelState "" emptyLineEditSelection))))

propCommandClickLabelChoosesEdgeComposeLabel :: Property
propCommandClickLabelChoosesEdgeComposeLabel =
  property $
    case find picksLabel clickGrid of
      Just point ->
        editorFocus (clickLabel point)
          === Just (Focus [rawChildLabel] defaultFocusState)
      Nothing ->
        counterexample "no coordinate cmd+clicked an edge label" False
  where
    pending =
      testEditor rawInsertDocument (Just (Focus [] (testPendingLabelState "" emptyLineEditSelection)))
    clickLabel (x, y) =
      execState
        (handlePointer (PointerDown x y commandModifiers) (rawInsertHandler (editorFocus pending)))
        pending
    picksLabel point =
      editorFocus (clickLabel point) == Just (Focus [rawChildLabel] defaultFocusState)
    clickGrid =
      [(x, y) | x <- [0, 2 .. 240], y <- [0, 2 .. 240]]

propRawCycleProjectsCollapsedAndExpanded :: Property
propRawCycleProjectsCollapsedAndExpanded =
  conjoin
    [ rawEditorHandler (newEditor cycleDocument) `seq` property True
    , rawEditorHandler (setCollapsed [cycleLabel] False (newEditor cycleDocument)) `seq` property True
    ]

propRawNodeInsertCommitsNestedString :: Property
propRawNodeInsertCommitsNestedString =
  conjoin
    [ editorFocus pending === Just (Focus [rawChildLabel] (testPendingLabelState "" emptyLineEditSelection))
    , editorFocus labeled === Just (Focus [rawChildLabel, rawInsertedLabel] (testPendingValueState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus [rawChildLabel, rawInsertedLabel] (testPendingValueState "delta" deltaSelection))
    , resolvePath insertedContext [rawChildLabel, rawInsertedLabel] === Just (VString "delta")
    , editorFocus inserted === Just (Focus [rawChildLabel, rawInsertedLabel] (testFocusState deltaSelection))
    ]
  where
    pending =
      execState
        (handleInsert (rawInsertHandler (Just (Focus [rawChildLabel] defaultFocusState))))
        (testEditor rawInsertDocument (Just (Focus [rawChildLabel] defaultFocusState)))
    labeled =
      chooseEdgeComposeLabel [rawChildLabel] rawInsertedLabel pending
    typed =
      execState
        (handleKey (TextInput "delta") (rawInsertHandler (editorFocus labeled)))
        labeled
    inserted =
      execState
        (handleKey enterKey (rawInsertHandler (editorFocus typed)))
        typed
    insertedContext =
      documentContext (editorDocument inserted) []
    deltaSelection =
      lineEditSelectionAtEnd "delta"

propClampScrollOffset :: Property
propClampScrollOffset =
  conjoin
    [ pointY (clampScrollOffset (Point 0 50) viewport (Size 100 200)) === 50
    , pointY (clampScrollOffset (Point 0 150) viewport (Size 100 200)) === 100
    , pointY (clampScrollOffset (Point 0 (-10)) viewport (Size 100 200)) === 0
    , pointX (clampScrollOffset (Point 50 0) viewport (Size 200 100)) === 50
    , pointX (clampScrollOffset (Point 150 0) viewport (Size 200 100)) === 100
    , pointX (clampScrollOffset (Point (-10) 0) viewport (Size 200 100)) === 0
    ]
  where
    viewport = Rect 0 0 100 100

data ScrollTest = ScrollTest
  { scrollTestOffset :: Point
  }
  deriving (Eq, Show)

emptyScrollTest :: ScrollTest
emptyScrollTest =
  ScrollTest (Point 0 0)

scrollTestViewport :: Rect
scrollTestViewport =
  Rect 0 0 200 100

scrollTestHandler :: Point -> Handler (State ScrollTest)
scrollTestHandler =
  scrollTestHandlerWithContent scrollTestContent

scrollTestContent :: Halay TestRender TestRender (Handler (State ScrollTest))
scrollTestContent =
  column [leaf (pure (Size 100 300)) (\_ -> pure mempty)]

scrollTestHandlerWithContent
  :: Halay TestRender TestRender (Handler (State ScrollTest))
  -> Point
  -> Handler (State ScrollTest)
scrollTestHandlerWithContent content offset =
  runTestRender $ do
    measured <-
      measureHalay
        ( scrollViewport
            (pure offset)
            (gets scrollTestOffset)
            (\next -> modify (\state -> state {scrollTestOffset = next}))
            content
        )
    placeMeasured measured (rootPlacement scrollTestViewport)

propScrollViewportWheelInsideViewport :: Property
propScrollViewportWheelInsideViewport =
  scrollTestOffset afterWheel === Point 0 16
  where
    afterWheel =
      execState
        (handleWheel scrollWheelInside (scrollTestHandler (Point 0 0)))
        emptyScrollTest
    scrollWheelInside =
      Wheel
        { wheelX = 100
        , wheelY = 50
        , wheelDeltaX = 0
        , wheelDeltaY = -1
        , wheelDeltaMode = 1
        , wheelModifiers = noModifiers
        }

propScrollViewportTrackpadScrollDirection :: Property
propScrollViewportTrackpadScrollDirection =
  scrollTestOffset afterWheel === Point 0 3
  where
    afterWheel =
      execState
        (handleWheel scrollWheelTrackpad (scrollTestHandler (Point 0 0)))
        emptyScrollTest
    scrollWheelTrackpad =
      Wheel
        { wheelX = 100
        , wheelY = 50
        , wheelDeltaX = 0
        , wheelDeltaY = 3
        , wheelDeltaMode = 0
        , wheelModifiers = noModifiers
        }

propScrollViewportWheelAccumulates :: Property
propScrollViewportWheelAccumulates =
  scrollTestOffset afterWheels === Point 0 32
  where
    handler = scrollTestHandler (Point 0 0)
    scrollWheelInside =
      Wheel
        { wheelX = 100
        , wheelY = 50
        , wheelDeltaX = 0
        , wheelDeltaY = -1
        , wheelDeltaMode = 1
        , wheelModifiers = noModifiers
        }
    afterWheels =
      execState
        ( do
            handleWheel scrollWheelInside handler
            handleWheel scrollWheelInside handler
        )
        emptyScrollTest

propScrollViewportWheelClampsToContent :: Property
propScrollViewportWheelClampsToContent =
  pointY (scrollTestOffset afterWheel) === 200
  where
    afterWheel =
      execState
        (handleWheel scrollWheelClamp (scrollTestHandler (Point 0 250)))
        emptyScrollTest
    scrollWheelClamp =
      Wheel
        { wheelX = 100
        , wheelY = 50
        , wheelDeltaX = 0
        , wheelDeltaY = -100
        , wheelDeltaMode = 1
        , wheelModifiers = noModifiers
        }

scrollTestContentIO :: Halay IO IO (Handler IO)
scrollTestContentIO =
  column [leaf (pure (Size 100 300)) (\_ -> pure mempty)]

propScrollChildPlacementYMoves :: Property
propScrollChildPlacementYMoves =
  ioProperty $ do
    origin <- traceScrollPlacement scrollTestViewport (Point 0 0) scrollTestContentIO
    scrolled <- traceScrollPlacement scrollTestViewport (Point 0 32) scrollTestContentIO
    let Rect {y = originY} = scrollTraceChildRect origin
    let Rect {y = scrolledY} = scrollTraceChildRect scrolled
    pure $
      counterexample
        ("origin=" <> show origin <> " scrolled=" <> show scrolled)
        ( conjoin
            [ property (scrolledY < originY)
            , pointY (scrollTraceAppliedOffset scrolled) === 32
            ]
        )

scrollClipTestViewport :: Rect
scrollClipTestViewport =
  Rect 0 0 200 100

scrollClipTestLayout :: Halay IO IO (Handler IO)
scrollClipTestLayout =
  box
    defaultBox
      { boxDirection = TopToBottom
      , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
      }
    [ scrollViewport
        (pure (Point 0 32))
        (pure (Point 0 32))
        (const (pure ()))
        scrollTestContentIO
    ]

propScrollClipViewportClampsOffset :: Property
propScrollClipViewportClampsOffset =
  ioProperty $ do
    trace <- traceScrollPlacement scrollClipTestViewport (Point 0 32) scrollClipTestLayout
    let layoutHeight = sizeHeight (scrollTraceLayoutContentSize trace)
    let clipHeight = height (scrollTraceViewportClip trace)
    let maxScrollY = max 0 (layoutHeight - clipHeight)
    pure $
      counterexample (show trace) $
        conjoin
          [ pointY (scrollTraceAppliedOffset trace) === min 32 maxScrollY
          , pointY (scrollTraceChildOffset trace) === negate (min 32 maxScrollY)
          , y (scrollTraceChildRect trace) === negate (min 32 maxScrollY)
          ]

propScrollClampUsesClipNotLayoutRect :: Property
propScrollClampUsesClipNotLayoutRect =
  let viewportClip = Rect 0 0 998 196
      layoutRect = Rect 0 0 998 265
      content = Size 998 265
      offset = Point 0 69
   in conjoin
        [ pointY (clampScrollOffset offset viewportClip content) === 69
        , pointY (clampScrollOffset offset layoutRect content) === 0
        ]

propRawEdgeInsertCommitsSiblingString :: Property
propRawEdgeInsertCommitsSiblingString =
  conjoin
    [ editorFocus pending === Just (Focus [] (testPendingLabelState "" emptyLineEditSelection))
    , editorFocus labeled === Just (Focus [rawInsertedLabel] (testPendingValueState "" emptyLineEditSelection))
    , editorFocus typed === Just (Focus [rawInsertedLabel] (testPendingValueState "epsilon" epsilonSelection))
    , resolvePath insertedContext [rawInsertedLabel] === Just (VString "epsilon")
    , editorFocus inserted === Just (Focus [rawInsertedLabel] (testFocusState epsilonSelection))
    ]
  where
    pending =
      execState
        (handleInsert (rawInsertHandler (Just (Focus [rawStringLabel] defaultFocusState))))
        (testEditor rawInsertDocument (Just (Focus [rawStringLabel] defaultFocusState)))
    labeled =
      chooseEdgeComposeLabel [] rawInsertedLabel pending
    typed =
      execState
        (handleKey (TextInput "epsilon") (rawInsertHandler (editorFocus labeled)))
        labeled
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
    placeMeasured measured (rootPlacement (Rect 0 0 800 600))

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
    placeMeasured measured (rootPlacement (Rect 0 0 800 600))

rawEditorHandler :: Editor -> Handler (State Editor)
rawEditorHandler editor =
  runTestRender $ do
    measured <- measureHalay (rawEditorLayout editor)
    placeMeasured measured (rootPlacement (Rect 0 0 800 600))

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
          (graphPanelNoCompose rawInsertDocument)
            { graphPanelDrag = drag
            , graphPanelDragStart = \newDrag -> modify (\state -> state {graphDragTestDrag = Just newDrag})
            , graphPanelDragMove = \position -> modify (\state -> state {graphDragTestMoved = Just position})
            , graphPanelDragEnd = modify (\state -> state {graphDragTestEnded = True})
            }
    placeMeasured measured (rootPlacement (Rect 0 0 320 240))

graphComposeHandler :: Maybe Focus -> Handler (State Editor)
graphComposeHandler focus =
  runTestRender $ do
    measured <-
      measureHalay $
        graphPanel
          (graphSnapshot (testEditor rawInsertDocument focus) Nothing)
          emptyGraphViewport
          emptyGraphLayout
          (graphPanelNoCompose rawInsertDocument)
            { graphPanelComposeMode = focus >>= focusUnderSelection . focusState
            , graphPanelComposePickLabel =
                \label ->
                  case composeParentPath focus of
                    Just parentPath -> modify (chooseEdgeComposeLabel parentPath label)
                    Nothing -> pure ()
            , graphPanelComposePickValue =
                \value ->
                  case focus >>= focusUnderSelection . focusState of
                    Just UnderValue ->
                      modify (replaceFocusedSpot rawInsertedLabel value)
                    _ -> pure ()
            }
    placeMeasured measured (rootPlacement (Rect 0 0 320 240))

graphInteractionHandler :: GraphInteractionTest -> Handler (State GraphInteractionTest)
graphInteractionHandler state =
  runTestRender $ do
    measured <-
      measureHalay $
        graphPanel
          (graphSnapshot (newEditor rawInsertDocument) Nothing)
          (graphInteractionTestViewport state)
          emptyGraphLayout
          (graphPanelNoCompose rawInsertDocument)
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
    placeMeasured measured (rootPlacement (Rect 0 0 320 240))

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

testPendingLabelState :: String -> LineEditSelection -> FocusState
testPendingLabelState string selection =
  defaultFocusState
    { focusPendingEdit = Just (PendingEdit string selection)
    , focusUnderSelection = Just UnderLabel
    }

testPendingValueState :: String -> LineEditSelection -> FocusState
testPendingValueState string selection =
  defaultFocusState
    { focusPendingEdit = Just (PendingEdit string selection)
    , focusUnderSelection = Just UnderValue
    }

enterKey :: KeyEvent
enterKey =
  KeyCode noModifiers KeyCode.enter

commaKey :: KeyEvent
commaKey =
  KeyCode noModifiers KeyCode.comma

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
