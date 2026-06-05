module Progred.Frame
  ( Frame (..)
  , KeyEvent (..)
  , KeyHandler
  , PointerEvent (..)
  , PointerHandler
  , onKey
  , onPointer
  , runKeyHandlers
  , runPointerHandlers
  ) where

import Data.Word (Word32)

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

type PointerHandler actionM = PointerEvent -> Maybe (actionM ())

type KeyHandler actionM = KeyEvent -> Maybe (actionM ())

data Frame actionM = Frame
  { pointerHandlers :: [PointerHandler actionM]
  , keyHandlers :: [KeyHandler actionM]
  }

instance Semigroup (Frame actionM) where
  left <> right =
    Frame
      { pointerHandlers = pointerHandlers left <> pointerHandlers right
      , keyHandlers = keyHandlers left <> keyHandlers right
      }

instance Monoid (Frame actionM) where
  mempty = Frame [] []

onPointer :: PointerHandler actionM -> Frame actionM
onPointer handler =
  mempty {pointerHandlers = [handler]}

onKey :: KeyHandler actionM -> Frame actionM
onKey handler =
  mempty {keyHandlers = [handler]}

runPointerHandlers
  :: Monad actionM
  => PointerEvent
  -> Frame actionM
  -> actionM ()
runPointerHandlers event frame =
  runFirst event (reverse (pointerHandlers frame))

runKeyHandlers
  :: Monad actionM
  => KeyEvent
  -> Frame actionM
  -> actionM ()
runKeyHandlers event frame =
  runFirst event (keyHandlers frame)

runFirst
  :: Monad m
  => event
  -> [event -> Maybe (m ())]
  -> m ()
runFirst _event [] =
  pure ()
runFirst event (handler : rest) =
  case handler event of
    Nothing -> runFirst event rest
    Just action -> action
