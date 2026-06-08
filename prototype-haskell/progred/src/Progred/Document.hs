module Progred.Document
  ( Document (..)
  ) where

import Data.UUID.Types (UUID)
import Progred.MapGraph

data Document = Document
  { documentRoot :: UUID
  , documentGraph :: MapGraph
  }
