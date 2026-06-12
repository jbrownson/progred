module Progred.MapGraph
  ( MapGraph
  , deleteEdgeValue
  , setEdgeValue
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Maybe (fromMaybe)
import Data.UUID.Types (UUID)
import Progred.Graph (Edges, Value)

type MapGraph = Map UUID Edges

setEdgeValue :: UUID -> UUID -> Value -> MapGraph -> MapGraph
setEdgeValue source label value =
  Map.alter (Just . Map.insert label value . fromMaybe Map.empty) source

deleteEdgeValue :: UUID -> UUID -> MapGraph -> MapGraph
deleteEdgeValue source label =
  Map.adjust (Map.delete label) source
