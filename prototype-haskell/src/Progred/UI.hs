module Progred.UI
  ( DrawCommand (..)
  , FocusTarget (..)
  , Frame (..)
  , Insets (..)
  , KeyEvent (..)
  , Point (..)
  , PointerEvent (..)
  , Rect (..)
  , draw
  , fillRect
  , fillText
  , fillTextMiddle
  , onKey
  , onPointer
  , insetRect
  , rectContains
  , runKeyHandlers
  , runPointerHandlers
  , strokeRect
  ) where

import Data.Foldable (asum)
import Data.Word (Word32)

data Point = Point
  { pointX :: Double
  , pointY :: Double
  }

data Rect = Rect
  { x :: Double
  , y :: Double
  , width :: Double
  , height :: Double
  }

data Insets = Insets
  { insetTop :: Double
  , insetRight :: Double
  , insetBottom :: Double
  , insetLeft :: Double
  }

data DrawCommand
  = FillRect Rect String
  | StrokeRect Rect String Double
  | FillText Point String String
  | FillTextMiddle Point String String

data PointerEvent
  = PointerDown
  { pointerX :: Double
  , pointerY :: Double
  }
  | PointerMove
  { pointerX :: Double
  , pointerY :: Double
  }
  | PointerUp
  { pointerX :: Double
  , pointerY :: Double
  }

data KeyEvent
  = KeyCode Word32
  | TextInput String

data FocusTarget world = FocusTarget
  { focusTargetIsFocused :: Bool
  , focusTargetFocus :: world -> world
  }

type PointerHandler world m = world -> PointerEvent -> Maybe (m world)

type KeyHandler world m = world -> KeyEvent -> Maybe (m world)

data Frame world m = Frame
  { draws :: [DrawCommand]
  , pointerHandlers :: [PointerHandler world m]
  , keyHandlers :: [KeyHandler world m]
  }

instance Semigroup (Frame world m) where
  left <> right =
    Frame
      { draws = draws left <> draws right
      , pointerHandlers = pointerHandlers left <> pointerHandlers right
      , keyHandlers = keyHandlers left <> keyHandlers right
      }

instance Monoid (Frame world m) where
  mempty = Frame [] [] []

draw :: DrawCommand -> Frame world m
draw command =
  mempty {draws = [command]}

fillRect :: Rect -> String -> Frame world m
fillRect rect color =
  draw (FillRect rect color)

strokeRect :: Rect -> String -> Double -> Frame world m
strokeRect rect color lineWidth =
  draw (StrokeRect rect color lineWidth)

fillText :: Point -> String -> String -> Frame world m
fillText point color string =
  draw (FillText point color string)

fillTextMiddle :: Point -> String -> String -> Frame world m
fillTextMiddle point color string =
  draw (FillTextMiddle point color string)

onPointer :: PointerHandler world m -> Frame world m
onPointer handler =
  mempty {pointerHandlers = [handler]}

onKey :: KeyHandler world m -> Frame world m
onKey handler =
  mempty {keyHandlers = [handler]}

runPointerHandlers :: Monad m => PointerEvent -> Frame world m -> world -> m world
runPointerHandlers event frame world =
  runFirst world event (reverse (pointerHandlers frame))

runKeyHandlers :: Monad m => KeyEvent -> Frame world m -> world -> m world
runKeyHandlers event frame world =
  runFirst world event (keyHandlers frame)

runFirst
  :: Monad m
  => world
  -> event
  -> [world -> event -> Maybe (m world)]
  -> m world
runFirst world event handlers =
  case asum (fmap (\handler -> handler world event) handlers) of
    Nothing -> pure world
    Just update -> update

rectContains :: Rect -> Double -> Double -> Bool
rectContains Rect {x, y, width, height} px py =
  px >= x
    && px <= x + width
    && py >= y
    && py <= y + height

insetRect :: Insets -> Rect -> Rect
insetRect Insets {insetTop, insetRight, insetBottom, insetLeft} Rect {x, y, width, height} =
  Rect
    { x = x + insetLeft
    , y = y + insetTop
    , width = width - insetLeft - insetRight
    , height = height - insetTop - insetBottom
    }
