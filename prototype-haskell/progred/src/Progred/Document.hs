module Progred.Document
  ( Document (..)
  , documentContext
  ) where

import Progred.Graph
import Progred.GraphContext
import Progred.MapGraph

data Document = Document
  { documentRoot :: UUID
  , documentGraph :: MapGraph
  }

documentContext :: Document -> [MapGraph] -> GraphContext
documentContext document libraries =
  GraphContext
    { contextRoot = documentRoot document
    , contextGraph = documentGraph document
    , contextLibraries = libraries
    }
