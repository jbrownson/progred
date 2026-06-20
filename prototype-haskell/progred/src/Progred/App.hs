module Progred.App
  ( AppM
  , Model
  , initialModel
  , runAppM
  , stepGraphLayoutFrame
  , toggleDebugLayoutRects
  , toggleGraphView
  , view
  ) where

import Control.Monad.Trans.State.Strict (State, modify, runState, state)
import qualified Data.Map.Strict as Map
import Data.Word (Word32)
import qualified Data.UUID.Types as UUID
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
import System.Random (mkStdGen, randoms)

data Model = Model
  { modelEditor :: Editor
  , modelDebugLayoutRects :: Bool
  , modelShowGraph :: Bool
  , modelGraphLayout :: GraphView.GraphLayout
  , modelFreshUUIDs :: [UUID]
  }

type AppM = State Model

runAppM :: AppM a -> Model -> (a, Model)
runAppM = runState

initialModel :: Model
initialModel =
  Model
    { modelEditor =
        newEditor sampleDocument
    , modelDebugLayoutRects = False
    , modelShowGraph = False
    , modelGraphLayout = GraphView.emptyGraphLayout
    , modelFreshUUIDs = seededUUIDs 20260615
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
          }
    )

stepGraphLayoutFrame :: AppM Bool
stepGraphLayoutFrame =
  state $ \model ->
    if modelShowGraph model
      then
        let snapshot = GraphView.graphSnapshot (modelEditor model)
            layout = GraphView.stepGraphLayout snapshot (modelGraphLayout model)
         in (True, model {modelGraphLayout = layout})
      else (False, model)

view :: Canvas.Canvas renderM => Viewport -> Model -> renderM (Handler AppM)
view viewport model = do
  Canvas.fillRect viewportRect "#fbfbfa"
  measured <- measureHalay appLayout
  placeMeasured measured viewportRect
  where
    viewportRect = Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)
    editor = modelEditor model
    appLayout =
      withLayoutDebug (modelDebugLayoutRects model) $
        decorate appHandler $
          workspaceLayout
    workspaceLayout =
      if modelShowGraph model
        then
          box
            defaultBox
              { boxDirection = LeftToRight
              , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
              }
            [ sized (Sizing (Percent 0.6) (Fill unbounded)) documentLayout
            , sized (Sizing (Percent 0.4) (Fill unbounded)) graphLayout
            ]
        else documentLayout
    documentLayout =
      box
        defaultBox
          { boxDirection = TopToBottom
          , boxPadding = Insets 12 12 12 12
          , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
          }
        [projectEditor (focusedProjection (listProjection `over` rawProjection)) editor editEditor freshUUID]
    graphLayout =
      GraphView.graphPanel
        (GraphView.graphSnapshot editor)
        (modelGraphLayout model)
    editEditor change =
      modify
        ( \current ->
            current {modelEditor = change (modelEditor current)}
        )
    freshUUID =
      state $ \current ->
        case modelFreshUUIDs current of
          fresh : rest -> (fresh, current {modelFreshUUIDs = rest})
          [] -> error "fresh UUID supply unexpectedly exhausted"
    appHandler _rect =
      pure $
        onPointer
          ( \event ->
              case event of
                PointerDown {} -> Just (editEditor (setFocus Nothing))
                _ -> Nothing
          )
          <> onDelete (editEditor deleteFocusedSpot)
          <> onKey
            ( \event ->
                case event of
                  KeyCode _modifiers code
                    | code == KeyCode.escape -> Just (editEditor (setFocus Nothing))
                  _ -> Nothing
            )

withLayoutDebug :: Canvas.Canvas renderM => Bool -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
withLayoutDebug enabled layout
  | enabled = debugRects drawDebugRect layout
  | otherwise = layout

drawDebugRect :: Canvas.Canvas renderM => Int -> Rect -> renderM (Handler actionM)
drawDebugRect depth rect = do
  Canvas.strokeRect rect (debugRectColor depth) 1
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

seededUUIDs :: Int -> [UUID]
seededUUIDs seed = wordsToUUIDs (randoms (mkStdGen seed))

wordsToUUIDs :: [Word32] -> [UUID]
wordsToUUIDs (word0 : word1 : word2 : word3 : rest) =
  UUID.fromWords word0 word1 word2 word3 : wordsToUUIDs rest
wordsToUUIDs _ = []
