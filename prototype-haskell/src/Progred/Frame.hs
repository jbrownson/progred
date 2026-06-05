module Progred.Frame
  ( Frame (..)
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

import Data.Word (Word32)
import qualified Progred.Canvas as Canvas
import Progred.Geometry

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

type PointerHandler actionM renderM = PointerEvent -> renderM (Maybe (actionM ()))

type KeyHandler actionM renderM = KeyEvent -> renderM (Maybe (actionM ()))

data Frame actionM renderM = Frame
  { renderFrame :: renderM ()
  , pointerHandlers :: [PointerHandler actionM renderM]
  , keyHandlers :: [KeyHandler actionM renderM]
  }

instance Applicative renderM => Semigroup (Frame actionM renderM) where
  left <> right =
    Frame
      { renderFrame = renderFrame left *> renderFrame right
      , pointerHandlers = pointerHandlers left <> pointerHandlers right
      , keyHandlers = keyHandlers left <> keyHandlers right
      }

instance Applicative renderM => Monoid (Frame actionM renderM) where
  mempty = Frame (pure ()) [] []

draw :: renderM () -> Frame actionM renderM
draw action =
  Frame action [] []

fillRect :: Canvas.Canvas renderM => Rect -> String -> Frame actionM renderM
fillRect rect color =
  draw (Canvas.fillRect rect color)

strokeRect :: Canvas.Canvas renderM => Rect -> String -> Double -> Frame actionM renderM
strokeRect rect color lineWidth =
  draw (Canvas.strokeRect rect color lineWidth)

fillText :: Canvas.Canvas renderM => Point -> String -> String -> Frame actionM renderM
fillText point color string =
  draw (Canvas.fillText point color string)

fillTextMiddle :: Canvas.Canvas renderM => Point -> String -> String -> Frame actionM renderM
fillTextMiddle point color string =
  draw (Canvas.fillTextMiddle point color string)

onPointer :: Applicative renderM => PointerHandler actionM renderM -> Frame actionM renderM
onPointer handler =
  mempty {pointerHandlers = [handler]}

onKey :: Applicative renderM => KeyHandler actionM renderM -> Frame actionM renderM
onKey handler =
  mempty {keyHandlers = [handler]}

runPointerHandlers
  :: (Monad actionM, Monad renderM)
  => PointerEvent
  -> Frame actionM renderM
  -> renderM (actionM ())
runPointerHandlers event frame =
  runFirst event (reverse (pointerHandlers frame))

runKeyHandlers
  :: (Monad actionM, Monad renderM)
  => KeyEvent
  -> Frame actionM renderM
  -> renderM (actionM ())
runKeyHandlers event frame =
  runFirst event (keyHandlers frame)

runFirst
  :: (Monad actionM, Monad renderM)
  => event
  -> [event -> renderM (Maybe (actionM ()))]
  -> renderM (actionM ())
runFirst _event [] =
  pure (pure ())
runFirst event (handler : rest) = do
  result <- handler event
  case result of
    Nothing -> runFirst event rest
    Just action -> pure action
