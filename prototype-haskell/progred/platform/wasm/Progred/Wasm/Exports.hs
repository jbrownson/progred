{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Wasm.Exports () where

import Data.Word (Word32)
import GHC.Wasm.Prim (JSString (JSString), fromJSString)
import qualified Main
import qualified Puri.KeyCode as KeyCode

foreign export javascript "start sync"
  start :: IO ()

foreign export javascript "onClick sync"
  onClick :: Word32 -> IO ()

foreign export javascript "onKeyDown sync"
  onKeyDown :: Word32 -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()

foreign export javascript "onTextInput sync"
  onTextInput :: JSString -> IO ()

foreign export javascript "toggleLayoutDebugRects sync"
  toggleLayoutDebugRects :: IO ()

foreign export javascript "toggleGraphView sync"
  toggleGraphView :: IO ()

foreign export javascript "onAnimationFrame sync"
  onAnimationFrame :: IO ()

foreign export javascript "onPointerDown sync"
  onPointerDown :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()

foreign export javascript "onPointerMove sync"
  onPointerMove :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()

foreign export javascript "onPointerUp sync"
  onPointerUp :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()

foreign export javascript "onResize sync"
  onResize :: Double -> Double -> IO ()

start :: IO ()
start = Main.main

onClick :: Word32 -> IO ()
onClick _ = Main.onKeyDown KeyCode.enter 0 0 0 0

onKeyDown :: Word32 -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onKeyDown = Main.onKeyDown

onTextInput :: JSString -> IO ()
onTextInput =
  Main.onTextInput . fromJSString

toggleLayoutDebugRects :: IO ()
toggleLayoutDebugRects =
  Main.toggleLayoutDebugRects

toggleGraphView :: IO ()
toggleGraphView =
  Main.toggleGraphView

onAnimationFrame :: IO ()
onAnimationFrame =
  Main.onAnimationFrame

onPointerDown :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerDown = Main.onPointerDown

onPointerMove :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerMove = Main.onPointerMove

onPointerUp :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerUp = Main.onPointerUp

onResize :: Double -> Double -> IO ()
onResize = Main.onResize
