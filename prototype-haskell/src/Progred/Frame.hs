module Progred.Frame
  ( DrawCommand (..)
  , Frame (..)
  , KeyEvent (..)
  , KeyHandler
  , PointerEvent (..)
  , PointerHandler
  , draw
  , fillRect
  , fillText
  , fillTextMiddle
  , onKey
  , onPointer
  , runKeyHandlers
  , runPointerHandlers
  , strokeRect
  ) where

import Data.Foldable (asum)
import Data.Word (Word32)
import Progred.Geometry

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

type PointerHandler m = PointerEvent -> Maybe (m ())

type KeyHandler m = KeyEvent -> Maybe (m ())

data Frame m = Frame
  { draws :: [DrawCommand]
  , pointerHandlers :: [PointerHandler m]
  , keyHandlers :: [KeyHandler m]
  }

instance Semigroup (Frame m) where
  left <> right =
    Frame
      { draws = draws left <> draws right
      , pointerHandlers = pointerHandlers left <> pointerHandlers right
      , keyHandlers = keyHandlers left <> keyHandlers right
      }

instance Monoid (Frame m) where
  mempty = Frame [] [] []

draw :: DrawCommand -> Frame m
draw command =
  mempty {draws = [command]}

fillRect :: Rect -> String -> Frame m
fillRect rect color =
  draw (FillRect rect color)

strokeRect :: Rect -> String -> Double -> Frame m
strokeRect rect color lineWidth =
  draw (StrokeRect rect color lineWidth)

fillText :: Point -> String -> String -> Frame m
fillText point color string =
  draw (FillText point color string)

fillTextMiddle :: Point -> String -> String -> Frame m
fillTextMiddle point color string =
  draw (FillTextMiddle point color string)

onPointer :: PointerHandler m -> Frame m
onPointer handler =
  mempty {pointerHandlers = [handler]}

onKey :: KeyHandler m -> Frame m
onKey handler =
  mempty {keyHandlers = [handler]}

runPointerHandlers :: Monad m => PointerEvent -> Frame m -> m ()
runPointerHandlers event frame =
  runFirst event (reverse (pointerHandlers frame))

runKeyHandlers :: Monad m => KeyEvent -> Frame m -> m ()
runKeyHandlers event frame =
  runFirst event (keyHandlers frame)

runFirst
  :: Monad m
  => event
  -> [event -> Maybe (m ())]
  -> m ()
runFirst event handlers =
  case asum (fmap (\handler -> handler event) handlers) of
    Nothing -> pure ()
    Just action -> action
