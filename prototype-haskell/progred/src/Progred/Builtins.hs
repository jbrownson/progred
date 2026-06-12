module Progred.Builtins
  ( headLabel
  , nameLabel
  , nilNode
  , tailLabel
  ) where

import Data.UUID.Types (UUID)
import qualified Data.UUID.Types as UUID

-- Well-known identities shared by every document: the bootstrap
-- stand-in for a library graph, which can adopt these same UUIDs later.
-- A list is a ref to nilNode or to a node with head/tail edges; the end
-- of a list is tail -> nilNode, so absence of tail is malformed rather
-- than meaningful.

nameLabel :: UUID
nameLabel =
  UUID.fromWords 0x2e915a01 0x25b64c82 0x8ae69677 0x768d2d1f

headLabel :: UUID
headLabel =
  UUID.fromWords 0x3af783fc 0xb17d4966 0x9f920f66 0x781afc88

tailLabel :: UUID
tailLabel =
  UUID.fromWords 0x5ef36278 0xd1b649fd 0xbf08d493 0x1e841b7b

nilNode :: UUID
nilNode =
  UUID.fromWords 0x3178ce82 0xd2044519 0x8ed19a0a 0x81b35666
