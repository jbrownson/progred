module Main
  ( main
  , onKeyDown
  , onPointerDown
  , onPointerMove
  , onPointerUp
  , onResize
  , onTextInput
  ) where

import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Data.Word (Word32)
import Progred.App
import qualified Progred.Canvas as Canvas
import Progred.Frame
import Progred.Viewport
import System.IO.Unsafe (unsafePerformIO)

data Runtime = Runtime
  { runtimeModel :: Model
  , runtimeFrame :: Frame AppM
  }

runtime :: IORef Runtime
runtime =
  unsafePerformIO
    ( newIORef
        Runtime
          { runtimeModel = initialModel
          , runtimeFrame = mempty
          }
    )
{-# NOINLINE runtime #-}

main :: IO ()
main =
  renderState

onResize :: Double -> Double -> IO ()
onResize _width _height =
  renderState

onPointerDown :: Double -> Double -> IO ()
onPointerDown px py =
  dispatchPointer (PointerDown px py)

onPointerMove :: Double -> Double -> IO ()
onPointerMove px py =
  dispatchPointer (PointerMove px py)

onPointerUp :: Double -> Double -> IO ()
onPointerUp px py =
  dispatchPointer (PointerUp px py)

dispatchPointer :: PointerEvent -> IO ()
dispatchPointer event = do
  Runtime {runtimeModel = model, runtimeFrame = frame} <- readIORef runtime
  let (_, updated) = runAppM (runPointerHandlers event frame) model
  writeIORef runtime Runtime {runtimeModel = updated, runtimeFrame = frame}
  renderState

onKeyDown :: Word32 -> IO ()
onKeyDown keyCode =
  dispatchKey (KeyCode keyCode)

onTextInput :: String -> IO ()
onTextInput string =
  dispatchKey (TextInput string)

dispatchKey :: KeyEvent -> IO ()
dispatchKey event = do
  Runtime {runtimeModel = model, runtimeFrame = frame} <- readIORef runtime
  let (_, updated) = runAppM (runKeyHandlers event frame) model
  writeIORef runtime Runtime {runtimeModel = updated, runtimeFrame = frame}
  renderState

renderState :: IO ()
renderState = do
  viewport <- Canvas.getViewport
  Runtime {runtimeModel = model} <- readIORef runtime
  Canvas.clearCanvas viewport
  frame <- currentFrame viewport model
  writeIORef runtime Runtime {runtimeModel = model, runtimeFrame = frame}

currentFrame :: Viewport -> Model -> IO (Frame AppM)
currentFrame =
  view
