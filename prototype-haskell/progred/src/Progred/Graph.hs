module Progred.Graph
  ( Edge (..)
  , UUID
  , Value (..)
  , Edges
  ) where

import Data.Map.Strict (Map)
import Data.UUID.Types (UUID)

data Edge = Edge
  { edgeSource :: UUID
  , edgeLabel :: UUID
  }
  deriving (Eq, Show)

data Value
  = VRef UUID
  | VBool Bool
  | VString String
  | VInt Integer
  | VFloat Double
  deriving (Eq, Show)

type Edges = Map UUID Value
