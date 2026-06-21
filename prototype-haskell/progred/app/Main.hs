module Main
  ( main
  , onAnimationFrame
  , onKeyDown
  , onPointerDown
  , onPointerMove
  , onPointerUp
  , onResize
  , onWheel
  , onTextInput
  , toggleGraphView
  , toggleLayoutDebugRects
  ) where

import Control.Monad (when)
import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Data.Word (Word32)
import qualified Puri.Canvas as Canvas
import Puri.Handler hiding (onWheel)
import qualified Puri.KeyCode as KeyCode
import Puri.Viewport
import Progred.App hiding (toggleGraphView)
import qualified Progred.App as App
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

onPointerDown :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerDown px py shift alt ctrl meta =
  dispatchPointer (PointerDown px py (keyModifiers shift alt ctrl meta))

onPointerMove :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerMove px py shift alt ctrl meta =
  dispatchPointer (PointerMove px py (keyModifiers shift alt ctrl meta))

onPointerUp :: Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onPointerUp px py shift alt ctrl meta =
  dispatchPointer (PointerUp px py (keyModifiers shift alt ctrl meta))

onWheel :: Double -> Double -> Double -> Double -> Word32 -> Word32 -> Word32 -> Word32 -> Word32 -> IO ()
onWheel px py deltaX deltaY deltaMode shift alt ctrl meta =
  dispatchWheel
    Wheel
      { wheelX = px
      , wheelY = py
      , wheelDeltaX = deltaX
      , wheelDeltaY = deltaY
      , wheelDeltaMode = deltaMode
      , wheelModifiers = keyModifiers shift alt ctrl meta
      }

dispatchPointer :: PointerEvent -> IO ()
dispatchPointer event =
  dispatchRuntime (\handler -> handlePointer event handler)

dispatchWheel :: WheelEvent -> IO ()
dispatchWheel event =
  dispatchRuntime (\handler -> handleWheel event handler)

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
dispatchKey event =
  dispatchRuntime $ \handler ->
    case keyHandler handler event of
      Just action -> action
      Nothing
        | isDeleteKey event -> handleDelete handler
        | isInsertKey event -> handleInsert handler
        | otherwise -> pure ()

isDeleteKey :: KeyEvent -> Bool
isDeleteKey event =
  case event of
    KeyCode _modifiers code ->
      code == KeyCode.delete || code == KeyCode.backspace
    _ -> False

isInsertKey :: KeyEvent -> Bool
isInsertKey event =
  case event of
    KeyCode modifiers code ->
      code == KeyCode.enter && not (hasModifier modifiers)
    _ -> False

toggleLayoutDebugRects :: IO ()
toggleLayoutDebugRects =
  dispatchRuntime (\_handler -> toggleDebugLayoutRects)

toggleGraphView :: IO ()
toggleGraphView =
  dispatchRuntime (\_handler -> App.toggleGraphView)

onAnimationFrame :: IO ()
onAnimationFrame = do
  Runtime {runtimeModel = model, runtimeHandler = handler} <- readIORef runtime
  (changed, updated) <- runAppM stepGraphLayoutFrame model
  writeIORef runtime Runtime {runtimeModel = updated, runtimeHandler = handler}
  when changed renderState

dispatchRuntime :: (Handler AppM -> AppM ()) -> IO ()
dispatchRuntime action = do
  Runtime {runtimeModel = model, runtimeHandler = handler} <- readIORef runtime
  (_, updated) <- runAppM (action handler) model
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
