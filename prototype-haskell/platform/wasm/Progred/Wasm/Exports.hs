{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Wasm.Exports () where

import Data.Word (Word32)
import qualified Main

foreign export javascript "start"
  start :: IO ()

foreign export javascript "onClick"
  onClick :: Word32 -> IO ()

start :: IO ()
start = Main.main

onClick :: Word32 -> IO ()
onClick = Main.hello
