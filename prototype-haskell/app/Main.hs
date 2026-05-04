module Main (main, hello) where

import Data.Word (Word32)
import Progred.App (clickedMessage, initialMessage)
import Progred.Platform (logClick, setRoot)

main :: IO ()
main = setRoot initialMessage

hello :: Word32 -> IO ()
hello n = do
  logClick n
  setRoot (clickedMessage n)
