module Progred.GraphContext
  ( GraphContext (..)
  , PathWalk (..)
  , lookupNode
  , pathEdge
  , resolvePath
  , walkPath
  ) where

import Control.Applicative ((<|>))
import qualified Data.Map.Strict as Map
import Progred.Graph
import Progred.MapGraph

data GraphContext = GraphContext
  { contextRoot :: UUID
  , contextGraph :: MapGraph
  , contextLibraries :: [MapGraph]
  }
  deriving (Eq, Show)

data PathWalk = PathWalk
  { walkedNodes :: [UUID]
  , walkedValue :: Value
  }
  deriving (Eq, Show)

lookupNode :: GraphContext -> UUID -> Maybe Edges
lookupNode context node =
  Map.lookup node (contextGraph context) <|> firstLibraryHit (contextLibraries context)
  where
    firstLibraryHit libraries =
      case libraries of
        [] -> Nothing
        library : rest -> Map.lookup node library <|> firstLibraryHit rest

-- Resolves a path from the root: the nodes entered at each step and the
-- value at the end. The root spot is a ref to the context root.
walkPath :: GraphContext -> [UUID] -> Maybe PathWalk
walkPath context =
  go [] (VRef (contextRoot context))
  where
    go nodes value [] = Just PathWalk {walkedNodes = reverse nodes, walkedValue = value}
    go nodes (VRef node) (label : rest) = do
      edges <- lookupNode context node
      next <- Map.lookup label edges
      go (node : nodes) next rest
    go _ _ (_ : _) = Nothing

resolvePath :: GraphContext -> [UUID] -> Maybe Value
resolvePath context path =
  walkedValue <$> walkPath context path

pathEdge :: GraphContext -> [UUID] -> Maybe Edge
pathEdge context path = do
  PathWalk {walkedNodes = nodes} <- walkPath context path
  Edge <$> lastMaybe nodes <*> lastMaybe path

lastMaybe :: [item] -> Maybe item
lastMaybe =
  foldl (\_ item -> Just item) Nothing
