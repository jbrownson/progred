module Progred.Document
  ( Document (..)
  , walkPath
  ) where

import qualified Data.Map.Strict as Map
import Progred.Graph
import Progred.MapGraph

data Document = Document
  { documentRoot :: UUID
  , documentGraph :: MapGraph
  }

-- Resolves a path from the root: the nodes entered at each step and the
-- value at the end. The root spot is a ref to the document root.
walkPath :: Document -> [UUID] -> Maybe ([UUID], Value)
walkPath document =
  go [] (VRef (documentRoot document))
  where
    go nodes value [] = Just (reverse nodes, value)
    go nodes (VRef node) (label : rest) = do
      edges <- Map.lookup node (documentGraph document)
      next <- Map.lookup label edges
      go (node : nodes) next rest
    go _ _ (_ : _) = Nothing
