module Progred.App
  ( ActiveSelection (..)
  , AppM
  , Model (..)
  , activeSelectionAfterEdit
  , cancelEscape
  , clearActiveSelection
  , deleteActiveSelection
  , initialModel
  , runAppM
  , stepGraphLayoutFrame
  , toggleDebugLayoutRects
  , toggleGraphView
  , view
  ) where

import Control.Monad.IO.Class (MonadIO, liftIO)
import Control.Monad.Trans.State.Strict (StateT, evalStateT, gets, modify, runStateT, state)

import Progred.FreshUUID (MonadFreshUUID (..), seededUUIDs)
import qualified Data.Map.Strict as Map
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.Projection
import qualified Progred.Render.Graph as GraphView
import Progred.Render.List
import Progred.Render.Raw
import qualified Puri.Canvas as Canvas
import Puri.Handler
import qualified Puri.KeyCode as KeyCode
import Puri.Viewport
import Puri.Widgets.ScrollViewport (scrollViewport)


data ActiveSelection
  = ActiveDocument Focus
  | ActiveGraph GraphView.GraphSelection
  | ActiveNone
  deriving (Eq, Show)

data Model = Model
  { modelEditor :: Editor
  , modelDebugLayoutRects :: Bool
  , modelShowGraph :: Bool
  , modelGraphLayout :: GraphView.GraphLayout
  , modelGraphViewport :: GraphView.GraphViewport
  , modelActiveSelection :: ActiveSelection
  , modelGraphDrag :: Maybe GraphView.GraphDrag
  , modelGraphPan :: Maybe GraphView.GraphPan
  , modelGraphEdgePress :: Maybe GraphView.GraphEdge
  , modelGraphPointerOrigin :: Maybe Point
  , modelGraphPointerMoved :: Bool
  , modelDocumentScroll :: Point
  }

type AppM = StateT Model IO

runAppM :: AppM a -> Model -> IO (a, Model)
runAppM = runStateT

initialModel :: Model
initialModel =
  Model
    { modelEditor =
        newEditor sampleDocument
    , modelDebugLayoutRects = False
    , modelShowGraph = False
    , modelGraphLayout = GraphView.emptyGraphLayout
    , modelGraphViewport = GraphView.emptyGraphViewport
    , modelActiveSelection = ActiveNone
    , modelGraphDrag = Nothing
    , modelGraphPan = Nothing
    , modelGraphEdgePress = Nothing
    , modelGraphPointerOrigin = Nothing
    , modelGraphPointerMoved = False
    , modelDocumentScroll = Point 0 0
    }

toggleDebugLayoutRects :: AppM ()
toggleDebugLayoutRects =
  modify
    ( \model ->
        model
          { modelDebugLayoutRects = not (modelDebugLayoutRects model)
          }
    )

toggleGraphView :: AppM ()
toggleGraphView =
  modify
    ( \model ->
        model
          { modelShowGraph = not (modelShowGraph model)
          , modelActiveSelection = ActiveNone
          , modelGraphDrag = Nothing
          , modelGraphPan = Nothing
          , modelGraphEdgePress = Nothing
          , modelGraphPointerOrigin = Nothing
          , modelGraphPointerMoved = False
          }
    )

stepGraphLayoutFrame :: AppM Bool
stepGraphLayoutFrame =
  state $ \model ->
    if modelShowGraph model
      then
        let snapshot = GraphView.graphSnapshot (modelEditor model) (activeGraphSelection (modelActiveSelection model))
            currentLayout = modelGraphLayout model
            steppedLayout = GraphView.stepGraphLayout snapshot currentLayout
            layout = preserveGraphDrag (modelGraphDrag model) currentLayout steppedLayout
         in (True, model {modelGraphLayout = layout})
      else
        ( False
        , model
            { modelGraphDrag = Nothing
            , modelGraphPan = Nothing
            , modelGraphEdgePress = Nothing
            , modelGraphPointerOrigin = Nothing
            , modelGraphPointerMoved = False
            }
        )

view :: (Canvas.Canvas renderM, MonadIO renderM) => Viewport -> Model -> renderM (Handler AppM)
view viewport model = do
  Canvas.fillRect viewportRect "#fbfbfa"
  measured <- measureHalay (appLayout model)
  handler <- placeMeasured measured (rootPlacement viewportRect)
  pure handler
  where
    getPlacementOffset = liftIO (evalStateT (gets modelDocumentScroll) model)
    viewportRect = Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)
    editor = modelEditor model
    editorContent =
      projectEditor
        (focusedProjection (listProjection `over` rawProjection))
        editor
        editEditor
        freshCell
        (GraphView.treeSecondaryHighlight editor (activeGraphSelection (modelActiveSelection model)))
    appLayout current =
      withLayoutDebug (modelDebugLayoutRects current) $
        decorate appHandler $
          workspaceLayout current
    workspaceLayout current =
      if modelShowGraph current
        then
          box
            defaultBox
              { boxDirection = LeftToRight
              , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
              }
            [ sized (Sizing (Percent 0.6) (Fill unbounded)) (documentLayout current)
            , sized (Sizing (Percent 0.4) (Fill unbounded)) graphLayout
            ]
        else documentLayout current
    documentLayout _current =
      box
        defaultBox
          { boxDirection = TopToBottom
          , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
          }
        [ scrollViewport
            getPlacementOffset
            (gets modelDocumentScroll)
            setDocumentScroll
            documentContent
        ]
    documentContent =
      box
        defaultBox
          { boxDirection = TopToBottom
          , boxPadding = Insets 12 12 12 12
          , boxSizing = Sizing (Fill unbounded) (Fit unbounded)
          }
        [editorContent]
    graphLayout =
      GraphView.graphPanel
        (GraphView.graphSnapshot editor (activeGraphSelection (modelActiveSelection model)))
        (modelGraphViewport model)
        (modelGraphLayout model)
        GraphView.GraphPanelActions
          { GraphView.graphPanelDrag = modelGraphDrag model
          , GraphView.graphPanelPan = modelGraphPan model
          , GraphView.graphPanelEdgePress = modelGraphEdgePress model
          , GraphView.graphPanelViewport = modelGraphViewport model
          , GraphView.graphPanelPointerOrigin = modelGraphPointerOrigin model
          , GraphView.graphPanelPointerMoved = modelGraphPointerMoved model
          , GraphView.graphPanelDocument = editorDocument editor
          , GraphView.graphPanelComposeMode = editorFocus editor >>= focusUnderSelection . focusState
          , GraphView.graphPanelComposePickLabel = composePickLabel
          , GraphView.graphPanelComposePickValue = composePickValue
          , GraphView.graphPanelDragStart = startGraphDrag
          , GraphView.graphPanelDragMove = moveGraphDrag
          , GraphView.graphPanelDragEnd = endGraphDrag
          , GraphView.graphPanelPanStart = startGraphPan
          , GraphView.graphPanelPanMove = moveGraphPan
          , GraphView.graphPanelPanEnd = endGraphPan
          , GraphView.graphPanelEdgePressStart = startGraphEdgePress
          , GraphView.graphPanelEdgePressEnd = endGraphEdgePress
          , GraphView.graphPanelSetViewport = setGraphViewport
          , GraphView.graphPanelInteractionStart = startGraphInteraction
          , GraphView.graphPanelInteractionMove = moveGraphInteraction
          , GraphView.graphPanelSetSelection = setGraphSelection
          }
    freshCell :: AppM UUID
    freshCell = freshUUID
    setDocumentScroll offset =
      modify (\current -> current {modelDocumentScroll = offset})
    editEditor = modify . editEditorModel
    composePickLabel label =
      case composeParentPath (editorFocus editor) of
        Just parentPath -> modify (editEditorModel (chooseEdgeComposeLabel parentPath label))
        Nothing -> pure ()
    composePickValue value =
      case editorFocus editor >>= focusUnderSelection . focusState of
        Just UnderValue -> do
          cell <- freshCell
          modify (editEditorModel (replaceFocusedSpot cell value))
        _ -> pure ()
    startGraphDrag drag =
      modify $ \current ->
        current
          { modelGraphDrag = Just drag
          , modelGraphLayout =
              GraphView.moveGraphNode
                (GraphView.graphDragNode drag)
                (GraphView.graphDragPosition drag)
                (modelGraphLayout current)
          }
    moveGraphDrag position =
      modify $ \current ->
        case modelGraphDrag current of
          Just drag ->
            current
              { modelGraphLayout =
                  GraphView.moveGraphNode
                    (GraphView.graphDragNode drag)
                    position
                    (modelGraphLayout current)
              }
          Nothing -> current
    endGraphDrag =
      modify
        ( \current ->
            current
              { modelGraphDrag = Nothing
              , modelGraphPointerOrigin = Nothing
              , modelGraphPointerMoved = False
              }
        )
    startGraphPan pointer =
      modify $ \current ->
        current
          { modelGraphPan = Just (GraphView.GraphPan pointer)
          }
    startGraphEdgePress edge =
      modify $ \current ->
        current {modelGraphEdgePress = Just edge}
    endGraphEdgePress =
      modify
        ( \current ->
            current
              { modelGraphEdgePress = Nothing
              , modelGraphPointerOrigin = Nothing
              , modelGraphPointerMoved = False
              }
        )
    moveGraphPan pointer =
      modify $ \current ->
        case modelGraphPan current of
          Just pan ->
            let (nextViewport, panState) =
                  GraphView.moveGraphPan pointer (modelGraphViewport current) pan
             in current {modelGraphViewport = nextViewport, modelGraphPan = Just panState}
          Nothing -> current
    endGraphPan =
      modify
        ( \current ->
            current
              { modelGraphPan = Nothing
              , modelGraphPointerOrigin = Nothing
              , modelGraphPointerMoved = False
              }
        )
    setGraphViewport nextViewport =
      modify (\current -> current {modelGraphViewport = nextViewport})
    startGraphInteraction pointer =
      modify
        ( \current ->
            current
              { modelGraphPointerOrigin = Just pointer
              , modelGraphPointerMoved = False
              }
        )
    moveGraphInteraction pointer =
      modify
        ( \current ->
            case (modelGraphPointerOrigin current, modelGraphPointerMoved current) of
              (Just origin, False)
                | GraphView.graphPointerExceededClickThreshold origin pointer ->
                    current {modelGraphPointerMoved = True}
              _ -> current
        )
    setGraphSelection selection =
      modify $ \current ->
        case selection of
          Just graph -> applyActiveSelection (ActiveGraph graph) current
          Nothing ->
            case modelActiveSelection current of
              ActiveGraph _ -> applyActiveSelection ActiveNone current
              _ -> current
    appHandler _placement =
      pure $
        onPointer
          ( \event ->
              case event of
                PointerDown {} -> Just clearActiveSelection
                _ -> Nothing
          )
          <> onDelete deleteActiveSelection
          <> onKey
            ( \event ->
                case event of
                  KeyCode _modifiers code
                    | code == KeyCode.escape -> Just cancelEscape
                  _ -> Nothing
            )

editEditorModel :: (Editor -> Editor) -> Model -> Model
editEditorModel change current =
  let after = change (modelEditor current)
      selection =
        activeSelectionAfterEdit
          (modelEditor current)
          after
          (modelActiveSelection current)
   in applyActiveSelection selection (current {modelEditor = after})

deleteActiveSelection :: AppM ()
deleteActiveSelection = do
  selection <- gets modelActiveSelection
  case selection of
    ActiveGraph graphSelection ->
      modify $ \current ->
        applyActiveSelection ActiveNone $
          current {modelEditor = GraphView.deleteGraphSelection graphSelection (modelEditor current)}
    _ ->
      modify (editEditorModel deleteFocusedSpot)

cancelEscape :: AppM ()
cancelEscape =
  modify $ \model ->
    case edgeComposeParent (editorFocus (modelEditor model)) of
      Just _ ->
        model
          { modelActiveSelection = ActiveNone
          , modelEditor = cancelEdgeCompose (modelEditor model)
          }
      Nothing -> applyActiveSelection ActiveNone model

clearActiveSelection :: AppM ()
clearActiveSelection =
  modify $ \model ->
    case editorFocus (modelEditor model) of
      Just Focus {focusState = FocusState {focusUnderSelection = Just _}} ->
        model {modelActiveSelection = ActiveNone}
      _ -> applyActiveSelection ActiveNone model

activeGraphSelection :: ActiveSelection -> Maybe GraphView.GraphSelection
activeGraphSelection selection =
  case selection of
    ActiveGraph graph -> Just graph
    _ -> Nothing

applyActiveSelection :: ActiveSelection -> Model -> Model
applyActiveSelection selection model =
  model
    { modelActiveSelection = selection
    , modelEditor =
        case selection of
          ActiveDocument focus -> setFocus (Just focus) (modelEditor model)
          _ -> setFocus Nothing (modelEditor model)
    }

activeSelectionAfterEdit :: Editor -> Editor -> ActiveSelection -> ActiveSelection
activeSelectionAfterEdit _ after current =
  case editorFocus after of
    Just focus -> ActiveDocument focus
    Nothing ->
      case current of
        ActiveGraph graph -> ActiveGraph graph
        _ -> ActiveNone

preserveGraphDrag :: Maybe GraphView.GraphDrag -> GraphView.GraphLayout -> GraphView.GraphLayout -> GraphView.GraphLayout
preserveGraphDrag drag current stepped =
  case drag of
    Nothing -> stepped
    Just GraphView.GraphDrag {GraphView.graphDragNode = draggedNode} ->
      case GraphView.graphLayoutPosition draggedNode current of
        Just position -> GraphView.moveGraphNode draggedNode position stepped
        Nothing -> stepped

withLayoutDebug :: Canvas.Canvas renderM => Bool -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
withLayoutDebug enabled layout
  | enabled = debugRects drawDebugRect layout
  | otherwise = layout

drawDebugRect :: Canvas.Canvas renderM => Int -> Placement -> renderM (Handler actionM)
drawDebugRect depth placement = do
  Canvas.strokeRect (placementRect placement) (debugRectColor depth) 1
  pure mempty

debugRectColor :: Int -> String
debugRectColor depth =
  debugRectColors !! (depth `mod` length debugRectColors)

debugRectColors :: [String]
debugRectColors =
  [ "#ff3860"
  , "#2f80ed"
  , "#00a676"
  , "#f2994a"
  , "#9b51e0"
  ]

sampleDocument :: Document
sampleDocument =
  Document
    { documentRoot = Just (ref 0)
    , documentGraph =
        Map.fromList
          [ ( uuid 0
            , node
                [ (nameLabel, VString "raw graph")
                , (uuid 4, VInt 42)
                , (uuid 5, VString "flag")
                , (uuid 6, ref 1)
                , (uuid 7, ref 11)
                ]
            )
          , ( uuid 1
            , node
                [ (nameLabel, VString "child")
                , (uuid 8, ref 0)
                ]
            )
          , ( uuid 2
            , node
                [ (nameLabel, VString "loop")
                , (uuid 8, ref 2)
                ]
            )
          , (uuid 11, cons (VString "alpha") (ref 12))
          , (uuid 12, cons (VFloat 3.14) (ref 13))
          , (uuid 13, cons (ref 2) (VRef nilNode))
          , (nilNode, node [(nameLabel, VString "nil")])
          ]
    }
  where
    ref = VRef . uuid
    uuid index = uuids !! index
    uuids = seededUUIDs 20260607
    node = Map.fromList
    cons element rest =
      node [(isaLabel, VRef listConsNode), (headLabel, element), (tailLabel, rest)]
