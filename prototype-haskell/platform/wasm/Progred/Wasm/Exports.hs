{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Wasm.Exports () where

import Data.Word (Word32)
import GHC.Wasm.Prim (JSString (JSString), fromJSString)
import qualified Main
import qualified Progred.KeyCode as KeyCode

foreign export javascript "start"
  start :: IO ()

foreign export javascript "onClick"
  onClick :: Word32 -> IO ()

foreign export javascript "onKeyDown"
  onKeyDown :: Word32 -> IO ()

foreign export javascript "onTextInput"
  onTextInput :: JSString -> IO ()

foreign export javascript "onPointerDown"
  onPointerDown :: Double -> Double -> IO ()

foreign export javascript "onPointerMove"
  onPointerMove :: Double -> Double -> IO ()

foreign export javascript "onPointerUp"
  onPointerUp :: Double -> Double -> IO ()

foreign export javascript "onResize"
  onResize :: Double -> Double -> IO ()

start :: IO ()
start = Main.main

onClick :: Word32 -> IO ()
onClick _ = Main.onKeyDown KeyCode.enter

onKeyDown :: Word32 -> IO ()
onKeyDown = Main.onKeyDown

onTextInput :: JSString -> IO ()
onTextInput =
  Main.onTextInput . fromJSString

onPointerDown :: Double -> Double -> IO ()
onPointerDown = Main.onPointerDown

onPointerMove :: Double -> Double -> IO ()
onPointerMove = Main.onPointerMove

onPointerUp :: Double -> Double -> IO ()
onPointerUp = Main.onPointerUp

onResize :: Double -> Double -> IO ()
onResize = Main.onResize
