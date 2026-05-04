module Main (main, hello) where

import Data.Word (Word32)
import Progred.Platform (logClick, setRoot)

initialMessage :: String
initialMessage = "hello from haskell"

clickedMessage :: Word32 -> String
clickedMessage n = initialMessage <> " - clicked " <> show n

main :: IO ()
main = setRoot initialMessage

hello :: Word32 -> IO ()
hello n = do
  logClick n
  setRoot (clickedMessage n)
