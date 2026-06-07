module Progred.MapGraph
  ( MapGraph
  , NodeDelta
  , MapGraphDelta
  , mapGraph
  , applyDelta
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.UUID.Types (UUID)
import Progred.Graph (Edges, Graph, Value)

type MapGraph = Map UUID Edges

type NodeDelta = Map UUID (Maybe Value)

type MapGraphDelta = Map UUID (Maybe NodeDelta)

mapGraph :: MapGraph -> Graph
mapGraph nodes source =
  Map.lookup source nodes

applyDelta :: MapGraphDelta -> MapGraph -> MapGraph
applyDelta delta graph =
  Map.foldlWithKey' applyNodeDelta graph delta

applyNodeDelta :: MapGraph -> UUID -> Maybe NodeDelta -> MapGraph
applyNodeDelta graph source maybeDelta =
  case maybeDelta of
    Nothing -> Map.delete source graph
    Just delta -> Map.alter (applyExistingNode delta) source graph
  where
    applyExistingNode delta Nothing =
      Just (applyNodeDeltaToEdges delta Map.empty)
    applyExistingNode delta (Just edges) =
      Just (applyNodeDeltaToEdges delta edges)

applyNodeDeltaToEdges :: NodeDelta -> Edges -> Edges
applyNodeDeltaToEdges delta edges =
  Map.foldlWithKey' applyEdgeDelta edges delta

applyEdgeDelta :: Edges -> UUID -> Maybe Value -> Edges
applyEdgeDelta edges label maybeTarget =
  case maybeTarget of
    Nothing -> Map.delete label edges
    Just target -> Map.insert label target edges
