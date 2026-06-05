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
  viewport <- Canvas.getViewport
  action <- runPointerHandlers event (currentFrame viewport model)
  let (_, updated) = runAppM action model
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
  viewport <- Canvas.getViewport
  action <- runKeyHandlers event (currentFrame viewport model)
  let (_, updated) = runAppM action model
  writeIORef state updated
  renderState

renderState :: IO ()
renderState = do
  viewport <- Canvas.getViewport
  model <- readIORef state
  let frame = currentFrame viewport model
  Canvas.clearCanvas viewport
  renderFrame frame

currentFrame :: Viewport -> Model -> Frame AppM IO
currentFrame =
  view
