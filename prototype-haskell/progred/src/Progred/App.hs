module Progred.App
  ( AppM
  , Model (..)
  , initialModel
  , runAppM
  , view
  ) where

import Control.Monad.Trans.State.Strict (State, modify, runState)
import qualified Data.Map.Strict as Map
import Data.Word (Word32)
import qualified Data.UUID.Types as UUID
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Graph
import Progred.MapGraph (MapGraphDelta, applyDelta)
import Progred.Render.Raw
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Viewport
import System.Random (mkStdGen, randoms)

data Model = Model
  { modelDocument :: Document
  , modelFocus :: Maybe Focus
  }

type AppM = State Model

runAppM :: AppM a -> Model -> (a, Model)
runAppM = runState

initialModel :: Model
initialModel =
  Model
    { modelDocument = sampleDocument
    , modelFocus = Nothing
    }

applyEdit :: MapGraphDelta -> AppM ()
applyEdit delta =
  modify editModel
  where
    editModel model =
      model
        { modelDocument = document {documentGraph = applyDelta delta graph}
        , modelFocus = transportFocus graph (documentRoot document) delta =<< modelFocus model
        }
      where
        document = modelDocument model
        graph = documentGraph document

setFocus :: Maybe Focus -> AppM ()
setFocus focus =
  modify (\model -> model {modelFocus = focus})

view :: Canvas.Canvas renderM => Viewport -> Model -> renderM (Handler AppM)
view viewport model = do
  Canvas.fillRect viewportRect "#fbfbfa"
  placeHalay viewportRect documentLayout
  where
    viewportRect = Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)
    env =
      RawEnv
        { rawApplyEdit = applyEdit
        , rawClearFocus = setFocus Nothing
        }
    cursor =
      FocusCursor
        { focusHere = modelFocus model
        , installFocus = setFocus . Just
        }
    documentLayout =
      decorate unfocusOnClick $
        box
          defaultBox
            { boxDirection = TopToBottom
            , boxPadding = Insets 12 12 12 12
            , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
            }
          [rawDocument env cursor (modelDocument model)]
    unfocusOnClick _rect =
      pure $ onPointer $ \event ->
        case event of
          PointerDown {} -> Just (setFocus Nothing)
          _ -> Nothing

sampleDocument :: Document
sampleDocument =
  Document
    { documentRoot = uuid 0
    , documentGraph =
        Map.fromList
          [ ( uuid 0
            , node
                [ (nameLabel, VString "raw graph")
                , (uuid 4, VInt 42)
                , (uuid 5, VBool True)
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
      node [(headLabel, element), (tailLabel, rest)]

seededUUIDs :: Int -> [UUID]
seededUUIDs seed = wordsToUUIDs (randoms (mkStdGen seed))

wordsToUUIDs :: [Word32] -> [UUID]
wordsToUUIDs (word0 : word1 : word2 : word3 : rest) =
  UUID.fromWords word0 word1 word2 word3 : wordsToUUIDs rest
wordsToUUIDs _ = []
