module Progred.MapGraph
  ( MapGraph
  , deleteEdgeValue
  , setEdgeValue
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Maybe (fromMaybe)
import Data.UUID.Types (UUID)
import Progred.Graph (Edge (..), Edges, Value)

type MapGraph = Map UUID Edges

setEdgeValue :: Edge -> Value -> MapGraph -> MapGraph
setEdgeValue Edge {edgeSource = source, edgeLabel = label} value =
  Map.alter (Just . Map.insert label value . fromMaybe Map.empty) source

deleteEdgeValue :: Edge -> MapGraph -> MapGraph
deleteEdgeValue Edge {edgeSource = source, edgeLabel = label} =
  Map.adjust (Map.delete label) source
