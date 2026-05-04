module Progred.App
  ( clickedMessage
  , initialMessage
  ) where

import Data.Word (Word32)

initialMessage :: String
initialMessage = "hello from haskell"

clickedMessage :: Word32 -> String
clickedMessage n = initialMessage <> " - clicked " <> show n
