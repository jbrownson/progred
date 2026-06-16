module Progred.Builtins
  ( headLabel
  , isaLabel
  , listBeforeLabel
  , listConsNode
  , nameLabel
  , nilNode
  , tailLabel
  ) where

import Data.UUID.Types (UUID)
import qualified Data.UUID.Types as UUID

-- Well-known identities shared by every document: the bootstrap
-- stand-in for a library graph, which can adopt these same UUIDs later.
-- A list is a ref to nilNode or to a node with isa -> listConsNode plus
-- head/tail edges. The end of a list is tail -> nilNode, so absence of tail
-- is malformed rather than meaningful. listBeforeLabel is a projection-local
-- path convention, not an edge list cells are expected to store.

isaLabel :: UUID
isaLabel =
  UUID.fromWords 0x0a61f4ef 0x09e6471d 0xa90be17e 0x52c9fbc7

nameLabel :: UUID
nameLabel =
  UUID.fromWords 0x2e915a01 0x25b64c82 0x8ae69677 0x768d2d1f

headLabel :: UUID
headLabel =
  UUID.fromWords 0x3af783fc 0xb17d4966 0x9f920f66 0x781afc88

tailLabel :: UUID
tailLabel =
  UUID.fromWords 0x5ef36278 0xd1b649fd 0xbf08d493 0x1e841b7b

listBeforeLabel :: UUID
listBeforeLabel =
  UUID.fromWords 0x47ff0fe3 0xf69541fe 0x93b2a527 0x3a2b6c10

listConsNode :: UUID
listConsNode =
  UUID.fromWords 0xd5c42475 0x3bd04f6f 0x99d9fb0d 0x74e5679f

nilNode :: UUID
nilNode =
  UUID.fromWords 0x3178ce82 0xd2044519 0x8ed19a0a 0x81b35666
