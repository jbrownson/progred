module Progred.MapGraph
  ( MapGraph
  , NodeDelta (..)
  , MapGraphDelta (..)
  , mapGraph
  , applyDelta
  , setEdgeDelta
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Maybe (fromMaybe)
import Data.UUID.Types (UUID)
import Progred.Graph (Edges, Graph, Value)

type MapGraph = Map UUID Edges

-- nodeDeltaResets drops the node's existing edges before the edge edits
-- apply (deleting the node when no edits follow), so deltas stay closed
-- under sequential composition: delete-then-edit is reset-with-edits.
data NodeDelta = NodeDelta
  { nodeDeltaResets :: Bool
  , nodeDeltaEdges :: Map UUID (Maybe Value)
  }
  deriving (Eq, Show)

newtype MapGraphDelta = MapGraphDelta (Map UUID NodeDelta)
  deriving (Eq, Show)

-- applyDelta (d1 <> d2) == applyDelta d2 . applyDelta d1
instance Semigroup NodeDelta where
  earlier <> later
    | nodeDeltaResets later = later
    | otherwise =
        NodeDelta
          { nodeDeltaResets = nodeDeltaResets earlier
          , nodeDeltaEdges = Map.union (nodeDeltaEdges later) (nodeDeltaEdges earlier)
          }

instance Semigroup MapGraphDelta where
  MapGraphDelta earlier <> MapGraphDelta later =
    MapGraphDelta (Map.unionWith (<>) earlier later)

instance Monoid MapGraphDelta where
  mempty = MapGraphDelta Map.empty

mapGraph :: MapGraph -> Graph
mapGraph nodes source =
  Map.lookup source nodes

setEdgeDelta :: UUID -> UUID -> Value -> MapGraphDelta
setEdgeDelta source label value =
  MapGraphDelta (Map.singleton source (NodeDelta False (Map.singleton label (Just value))))

applyDelta :: MapGraphDelta -> MapGraph -> MapGraph
applyDelta (MapGraphDelta delta) graph =
  Map.foldlWithKey' applyNodeDelta graph delta

applyNodeDelta :: MapGraph -> UUID -> NodeDelta -> MapGraph
applyNodeDelta graph source NodeDelta {nodeDeltaResets, nodeDeltaEdges}
  | nodeDeltaResets && Map.null nodeDeltaEdges = Map.delete source graph
  | nodeDeltaResets = Map.insert source (applyEdgeDeltas nodeDeltaEdges Map.empty) graph
  | otherwise = Map.alter (Just . applyEdgeDeltas nodeDeltaEdges . fromMaybe Map.empty) source graph

applyEdgeDeltas :: Map UUID (Maybe Value) -> Edges -> Edges
applyEdgeDeltas delta edges =
  Map.foldlWithKey' applyEdgeDelta edges delta

applyEdgeDelta :: Edges -> UUID -> Maybe Value -> Edges
applyEdgeDelta edges label maybeTarget =
  case maybeTarget of
    Nothing -> Map.delete label edges
    Just target -> Map.insert label target edges
