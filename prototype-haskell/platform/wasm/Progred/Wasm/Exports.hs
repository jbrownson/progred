{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Wasm.Exports () where

import Data.Word (Word32)
import qualified Main

foreign export javascript "start"
  start :: IO ()

foreign export javascript "onClick"
  onClick :: Word32 -> IO ()

foreign export javascript "onKeyDown"
  onKeyDown :: Word32 -> IO ()

foreign export javascript "onPointerDown"
  onPointerDown :: Double -> Double -> IO ()

foreign export javascript "onResize"
  onResize :: Double -> Double -> IO ()

start :: IO ()
start = Main.main

onClick :: Word32 -> IO ()
onClick _ = Main.onKeyDown 13

onKeyDown :: Word32 -> IO ()
onKeyDown = Main.onKeyDown

onPointerDown :: Double -> Double -> IO ()
onPointerDown = Main.onPointerDown

onResize :: Double -> Double -> IO ()
onResize = Main.onResize
