module Progred.Graph
  ( Edge (..)
  , ScalarKey (..)
  , UUID
  , Value (..)
  , Edges
  , scalarKey
  , valueHasScalarKey
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
  | VString String
  | VInt Integer
  | VFloat Double
  deriving (Eq, Show)

data ScalarKey
  = StringKey String
  | IntKey Integer
  | FloatKey Double
  deriving (Eq, Ord, Show)

type Edges = Map UUID Value

scalarKey :: Value -> Maybe ScalarKey
scalarKey value =
  case value of
    VString string -> Just (StringKey string)
    VInt integer -> Just (IntKey integer)
    VFloat double -> Just (FloatKey double)
    VRef _ -> Nothing

valueHasScalarKey :: Value -> ScalarKey -> Bool
valueHasScalarKey value key =
  scalarKey value == Just key
