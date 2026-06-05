module Main
  ( main
  , onKeyDown
  , onPointerDown
  , onPointerMove
  , onPointerUp
  , onResize
  , onTextInput
  ) where

import Data.Foldable (traverse_)
import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Data.Word (Word32)
import qualified Progred.Platform as Platform
import Progred.App
import Progred.Canvas
import Progred.Frame
import Progred.Viewport
import System.IO.Unsafe (unsafePerformIO)

state :: IORef Model
state = unsafePerformIO (newIORef initialModel)
{-# NOINLINE state #-}

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
  model <- readIORef state
  viewport <- getViewport
  let (_, updated) = runAppM (runPointerHandlers event (view viewport model)) model
  writeIORef state updated
  renderState

onKeyDown :: Word32 -> IO ()
onKeyDown keyCode =
  dispatchKey (KeyCode keyCode)

onTextInput :: String -> IO ()
onTextInput string =
  dispatchKey (TextInput string)

dispatchKey :: KeyEvent -> IO ()
dispatchKey event = do
  model <- readIORef state
  viewport <- getViewport
  let (_, updated) = runAppM (runKeyHandlers event (view viewport model)) model
  writeIORef state updated
  renderState

renderState :: IO ()
renderState = do
  viewport <- getViewport
  model <- readIORef state
  let frame = view viewport model
  Platform.clearCanvas (viewportWidth viewport) (viewportHeight viewport)
  traverse_ drawCanvas (draws frame)
