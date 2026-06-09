module Progred.App
  ( AppM
  , Model (..)
  , initialModel
  , runAppM
  , view
  ) where

import Control.Monad.Trans.State.Strict (State, runState)
import qualified Data.Map.Strict as Map
import Data.Word (Word32)
import qualified Data.UUID.Types as UUID
import Halay
import Progred.Document
import Progred.Graph
import Progred.Render.Raw
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Viewport
import System.Random (mkStdGen, randoms)

data Model = Model

type AppM = State Model

runAppM :: AppM a -> Model -> (a, Model)
runAppM = runState

initialModel :: Model
initialModel = Model

view :: Canvas.Canvas renderM => Viewport -> Model -> renderM (Handler AppM)
view viewport _model = do
  Canvas.fillRect viewportRect "#fbfbfa"
  _ <- placeHalay viewportRect sampleLayout
  pure mempty
  where
    viewportRect = Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)
    sampleLayout =
      box
        defaultBox
          { boxDirection = TopToBottom
          , boxPadding = Insets 12 12 12 12
          , boxWidth = Fill
          , boxHeight = Fill
          }
        [rawDocument sampleDocument]

sampleDocument :: Document
sampleDocument =
  Document
    { documentRoot = uuid 0
    , documentGraph =
        Map.fromList
          [ ( uuid 0
            , node
                [ (uuid 3, VString "raw graph")
                , (uuid 4, VInt 42)
                , (uuid 5, VBool True)
                , (uuid 6, ref 1)
                , (uuid 7, VList [VString "alpha", VFloat 3.14, ref 2])
                ]
            )
          , ( uuid 1
            , node
                [ (uuid 3, VString "child")
                , (uuid 8, ref 0)
                ]
            )
          , ( uuid 2
            , node
                [ (uuid 3, VString "loop")
                , (uuid 8, ref 2)
                ]
            )
          ]
    }
  where
    ref = VRef . uuid
    uuid index = uuids !! index
    uuids = seededUUIDs 20260607
    node = Map.fromList

seededUUIDs :: Int -> [UUID]
seededUUIDs seed = wordsToUUIDs (randoms (mkStdGen seed))

wordsToUUIDs :: [Word32] -> [UUID]
wordsToUUIDs (word0 : word1 : word2 : word3 : rest) =
  UUID.fromWords word0 word1 word2 word3 : wordsToUUIDs rest
wordsToUUIDs _ = []
