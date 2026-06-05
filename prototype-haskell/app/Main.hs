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
import Progred.Handler
import Progred.Viewport
import System.IO.Unsafe (unsafePerformIO)

data Runtime = Runtime
  { runtimeModel :: Model
  , runtimeHandler :: Handler AppM
  }

runtime :: IORef Runtime
runtime =
  unsafePerformIO
    ( newIORef
        Runtime
          { runtimeModel = initialModel
          , runtimeHandler = mempty
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
  Runtime {runtimeModel = model, runtimeHandler = handler} <- readIORef runtime
  let (_, updated) = runAppM (handlePointer event handler) model
  writeIORef runtime Runtime {runtimeModel = updated, runtimeHandler = handler}
  renderState

onKeyDown :: Word32 -> IO ()
onKeyDown keyCode =
  dispatchKey (KeyCode keyCode)

onTextInput :: String -> IO ()
onTextInput string =
  dispatchKey (TextInput string)

dispatchKey :: KeyEvent -> IO ()
dispatchKey event = do
  Runtime {runtimeModel = model, runtimeHandler = handler} <- readIORef runtime
  let (_, updated) = runAppM (handleKey event handler) model
  writeIORef runtime Runtime {runtimeModel = updated, runtimeHandler = handler}
  renderState

renderState :: IO ()
renderState = do
  viewport <- Canvas.getViewport
  Runtime {runtimeModel = model} <- readIORef runtime
  Canvas.clearCanvas viewport
  handler <- currentHandler viewport model
  writeIORef runtime Runtime {runtimeModel = model, runtimeHandler = handler}

currentHandler :: Viewport -> Model -> IO (Handler AppM)
currentHandler =
  view
