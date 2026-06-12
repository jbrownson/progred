module Progred.MapGraph
  ( MapGraph
  , MapGraphDelta (..)
  , NodeDelta (..)
  , mapGraph
  , applyDelta
  , deleteEdgeDelta
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
          , nodeDeltaEdges = Map.unionWith (\_ late -> late) (nodeDeltaEdges earlier) (nodeDeltaEdges later)
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
  edgeDelta source label (Just value)

deleteEdgeDelta :: UUID -> UUID -> MapGraphDelta
deleteEdgeDelta source label =
  edgeDelta source label Nothing

edgeDelta :: UUID -> UUID -> Maybe Value -> MapGraphDelta
edgeDelta source label slot =
  MapGraphDelta (Map.singleton source (NodeDelta False (Map.singleton label slot)))

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
applyEdgeDelta edges label slot =
  Map.alter (const slot) label edges
