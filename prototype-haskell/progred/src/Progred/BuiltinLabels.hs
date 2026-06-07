module Progred.BuiltinLabels
  ( nameLabel
  ) where

import Data.UUID.Types (UUID)
import qualified Data.UUID.Types as UUID

nameLabel :: UUID
nameLabel =
  UUID.fromWords 0x2e915a01 0x25b64c82 0x8ae69677 0x768d2d1f
