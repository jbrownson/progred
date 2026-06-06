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
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Viewport
import Progred.App
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

onKeyDown :: Word32 -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onKeyDown keyCode shift alt ctrl meta =
  dispatchKey (KeyCode (keyModifiers shift alt ctrl meta) keyCode)

keyModifiers :: Word32 -> Word32 -> Word32 -> Word32 -> KeyModifiers
keyModifiers shift alt ctrl meta =
  KeyModifiers
    { keyShift = shift /= 0
    , keyAlt = alt /= 0
    , keyCtrl = ctrl /= 0
    , keyMeta = meta /= 0
    }

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
