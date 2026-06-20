module Progred.GraphContext
  ( GraphContext (..)
  , PathWalk (..)
  , firstPathMatching
  , lookupNode
  , pathEdge
  , resolvePath
  , shortestPathToRef
  , walkPath
  ) where

import Control.Applicative ((<|>))
import qualified Data.Map.Strict as Map
import qualified Data.Set as Set
import Progred.Graph
import Progred.MapGraph

data GraphContext = GraphContext
  { contextRoot :: Maybe Value
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
-- value at the end. The root spot is stored directly on the document.
walkPath :: GraphContext -> [UUID] -> Maybe PathWalk
walkPath context path = do
  root <- contextRoot context
  go [] root path
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

shortestPathToRef :: GraphContext -> UUID -> Maybe [UUID]
shortestPathToRef context target = do
  root <- contextRoot context
  breadthFirst Set.empty [([], root)]
  where
    breadthFirst _ [] = Nothing
    breadthFirst visited ((path, value) : queue) =
      case value of
        VRef node
          | node == target -> Just path
          | Set.member node visited -> breadthFirst visited queue
          | otherwise -> do
              edges <- lookupNode context node
              let nextStates =
                    [ (path ++ [label], next)
                    | (label, next) <- Map.toList edges
                    ]
              breadthFirst (Set.insert node visited) (queue ++ nextStates)
        _ -> breadthFirst visited queue

firstPathMatching :: GraphContext -> (Value -> Bool) -> Maybe [UUID]
firstPathMatching context predicate = do
  root <- contextRoot context
  breadthFirst Set.empty [([], root)]
  where
    breadthFirst _ [] = Nothing
    breadthFirst visited ((path, value) : queue)
      | predicate value = Just path
      | otherwise =
          case value of
            VRef node
              | Set.member node visited -> breadthFirst visited queue
              | otherwise -> do
                  edges <- lookupNode context node
                  let nextStates =
                        [ (path ++ [label], next)
                        | (label, next) <- Map.toList edges
                        ]
                  breadthFirst (Set.insert node visited) (queue ++ nextStates)
            _ -> breadthFirst visited queue
