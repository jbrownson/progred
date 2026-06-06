module Progred.MapGid
  ( MapGid
  , emptyMapGid
  , mapGid
  , mapGidNodes
  , mapGidEdges
  , insertNode
  , deleteNode
  , setEdge
  , deleteEdge
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Maybe (fromMaybe)
import Progred.Gid (Gid (..), Nid (..), RoRw (..), edgesFromList)

newtype MapGid a = MapGid (Map a (Map a a))
  deriving (Eq, Show)

emptyMapGid :: MapGid a
emptyMapGid = MapGid Map.empty

mapGid :: Ord a => MapGid a -> Gid a
mapGid (MapGid nodes) =
  Gid $ \source ->
    case Map.lookup source nodes of
      Nothing -> Nothing
      Just edges -> Just (Nid Rw (edgesFromList (Map.toList edges)))

mapGidNodes :: MapGid a -> [a]
mapGidNodes (MapGid nodes) = Map.keys nodes

mapGidEdges :: Ord a => a -> MapGid a -> Maybe [(a, a)]
mapGidEdges source (MapGid nodes) = Map.toList <$> Map.lookup source nodes

insertNode :: Ord a => a -> MapGid a -> MapGid a
insertNode source (MapGid nodes) =
  MapGid (Map.alter (Just . fromMaybe Map.empty) source nodes)

deleteNode :: Ord a => a -> MapGid a -> MapGid a
deleteNode source (MapGid nodes) =
  MapGid (Map.delete source nodes)

setEdge :: Ord a => a -> a -> a -> MapGid a -> MapGid a
setEdge source label target (MapGid nodes) =
  MapGid (Map.alter setSource source nodes)
  where
    setSource maybeEdges =
      Just (Map.insert label target (fromMaybe Map.empty maybeEdges))

deleteEdge :: Ord a => a -> a -> MapGid a -> MapGid a
deleteEdge source label (MapGid nodes) =
  MapGid (Map.adjust (Map.delete label) source nodes)
