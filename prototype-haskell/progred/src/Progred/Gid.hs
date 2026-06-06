module Progred.Gid
  ( RoRw (..)
  , Edges
  , emptyEdges
  , edgesFromList
  , edge
  , edgeList
  , Nid (..)
  , Gid (..)
  , gidEdge
  , readOnlyGid
  , orElseGid
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map

data RoRw = Ro | Rw
  deriving (Eq, Ord, Show)

newtype Edges a = Edges (Map a a)
  deriving (Eq, Show)

emptyEdges :: Edges a
emptyEdges = Edges Map.empty

edgesFromList :: Ord a => [(a, a)] -> Edges a
edgesFromList entries = Edges (Map.fromList entries)

edge :: Ord a => a -> Edges a -> Maybe a
edge label (Edges edges) = Map.lookup label edges

edgeList :: Edges a -> [(a, a)]
edgeList (Edges edges) = Map.toList edges

data Nid a = Nid
  { nidRoRw :: RoRw
  , nidEdges :: Edges a
  }

newtype Gid a = Gid { runGid :: a -> Maybe (Nid a) }

gidEdge :: Ord a => Gid a -> a -> a -> Maybe a
gidEdge gid source label =
  runGid gid source >>= \nid ->
    edge label (nidEdges nid)

readOnlyGid :: Gid a -> Gid a
readOnlyGid gid =
  Gid $ \source ->
    case runGid gid source of
      Nothing -> Nothing
      Just nid -> Just nid { nidRoRw = Ro }

orElseGid :: Gid a -> Gid a -> Gid a
orElseGid left right =
  Gid $ \source ->
    case runGid left source of
      Nothing -> runGid right source
      Just nid -> Just nid
