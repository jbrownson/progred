module Main
  ( main
  , onKeyDown
  , onPointerDown
  , onResize
  ) where

import Data.Foldable (traverse_)
import Data.IORef (IORef, modifyIORef', newIORef, readIORef)
import Data.List (find)
import Data.Word (Word32)
import Progred.Platform
  ( clearCanvas
  , fillRect
  , fillText
  , strokeRect
  )
import System.IO.Unsafe (unsafePerformIO)

data FocusId
  = CounterButton
  | ResetButton
  deriving (Bounded, Enum, Eq)

data Model = Model
  { canvasWidth :: Double
  , canvasHeight :: Double
  , focus :: FocusId
  , count :: Int
  }

data Rect = Rect
  { x :: Double
  , y :: Double
  , width :: Double
  , height :: Double
  }

data Element = Element
  { elementFocus :: FocusId
  , elementRect :: Rect
  , elementLabel :: String
  }

data DrawCommand
  = FillRect Rect String
  | StrokeRect Rect String Double
  | FillText Double Double String String

state :: IORef Model
state = unsafePerformIO (newIORef initialModel)
{-# NOINLINE state #-}

initialModel :: Model
initialModel =
  Model
    { canvasWidth = 640
    , canvasHeight = 360
    , focus = CounterButton
    , count = 0
    }

main :: IO ()
main =
  renderState

onResize :: Double -> Double -> IO ()
onResize w h = do
  modifyIORef' state (\model -> model {canvasWidth = w, canvasHeight = h})
  renderState

onPointerDown :: Double -> Double -> IO ()
onPointerDown px py = do
  modifyIORef' state (\model ->
    case hitTest px py (layout model) of
      Nothing -> model
      Just element ->
        activate (elementFocus element) model {focus = elementFocus element})
  renderState

onKeyDown :: Word32 -> IO ()
onKeyDown keyCode = do
  modifyIORef' state (\model ->
    case keyCode of
      9 -> model {focus = nextFocus (focus model)}
      13 -> activate (focus model) model
      32 -> activate (focus model) model
      37 -> model {focus = previousFocus (focus model)}
      38 -> model {focus = previousFocus (focus model)}
      39 -> model {focus = nextFocus (focus model)}
      40 -> model {focus = nextFocus (focus model)}
      _ -> model)
  renderState

activate :: FocusId -> Model -> Model
activate CounterButton model =
  model {count = count model + 1}
activate ResetButton model =
  model {count = 0}

nextFocus :: FocusId -> FocusId
nextFocus focusId
  | focusId == maxBound = minBound
  | otherwise = succ focusId

previousFocus :: FocusId -> FocusId
previousFocus focusId
  | focusId == minBound = maxBound
  | otherwise = pred focusId

renderState :: IO ()
renderState = do
  model <- readIORef state
  let commands = render model
  clearCanvas (canvasWidth model) (canvasHeight model)
  traverse_ draw commands

render :: Model -> [DrawCommand]
render model =
  [ FillRect (Rect 0 0 (canvasWidth model) (canvasHeight model)) "#fbfbfa"
  , FillText 32 42 "#3f454d" "Haskell/Wasm canvas UI"
  , FillText 32 70 "#68707c" "State, focus, hit testing, and drawing are owned by Haskell."
  , FillText 32 110 "#3f454d" ("Count: " <> show (count model))
  ]
    <> concatMap (renderElement (focus model)) (layout model)

layout :: Model -> [Element]
layout _model =
  [ Element CounterButton (Rect 32 140 160 42) "Increment"
  , Element ResetButton (Rect 210 140 120 42) "Reset"
  ]

renderElement :: FocusId -> Element -> [DrawCommand]
renderElement active Element {elementFocus, elementRect, elementLabel} =
  [ FillRect elementRect background
  , StrokeRect elementRect border 2
  , FillText (x elementRect + 16) (y elementRect + 27) "#20242a" elementLabel
  ]
  where
    selected = active == elementFocus
    background = if selected then "#dbeaff" else "#ffffff"
    border = if selected then "#0a84ff" else "#c7cbd1"

hitTest :: Double -> Double -> [Element] -> Maybe Element
hitTest px py =
  find (\Element {elementRect} ->
    px >= x elementRect
      && px <= x elementRect + width elementRect
      && py >= y elementRect
      && py <= y elementRect + height elementRect)

draw :: DrawCommand -> IO ()
draw (FillRect Rect {x, y, width, height} color) =
  fillRect x y width height color
draw (StrokeRect Rect {x, y, width, height} color lineWidth) =
  strokeRect x y width height color lineWidth
draw (FillText x y color string) =
  fillText x y color string
