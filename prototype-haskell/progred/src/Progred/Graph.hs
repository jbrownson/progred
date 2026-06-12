module Progred.Graph
  ( UUID
  , Value (..)
  , Edges
  , Node
  , Graph
  ) where

import Data.Map.Strict (Map)
import Data.UUID.Types (UUID)

data Value
  = VRef UUID
  | VBool Bool
  | VString String
  | VInt Integer
  | VFloat Double
  deriving (Eq, Show)

type Edges = Map UUID Value

type Node = Edges

type Graph = UUID -> Maybe Node
