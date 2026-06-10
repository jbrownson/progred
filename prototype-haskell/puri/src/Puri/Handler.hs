module Puri.Handler
  ( Handler (..)
  , KeyEvent (..)
  , KeyHandler
  , KeyModifiers (..)
  , PointerEvent (..)
  , PointerHandler
  , handleKey
  , handlePointer
  , onKey
  , onPointer
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

data KeyModifiers = KeyModifiers
  { keyShift :: Bool
  , keyAlt :: Bool
  , keyCtrl :: Bool
  , keyMeta :: Bool
  }

data KeyEvent
  = KeyCode KeyModifiers Word32
  | TextInput String

type PointerHandler actionM = PointerEvent -> Maybe (actionM ())

type KeyHandler actionM = KeyEvent -> Maybe (actionM ())

data Handler actionM = Handler
  { pointerHandlers :: [PointerHandler actionM]
  , keyHandlers :: [KeyHandler actionM]
  }

instance Semigroup (Handler actionM) where
  left <> right =
    Handler
      { pointerHandlers = pointerHandlers left <> pointerHandlers right
      , keyHandlers = keyHandlers left <> keyHandlers right
      }

instance Monoid (Handler actionM) where
  mempty = Handler [] []

onPointer :: PointerHandler actionM -> Handler actionM
onPointer handler =
  mempty {pointerHandlers = [handler]}

onKey :: KeyHandler actionM -> Handler actionM
onKey handler =
  mempty {keyHandlers = [handler]}

-- Pointer handlers run last-registered first so the topmost-drawn widget
-- wins hit testing; key handlers keep registration order (focus order).
handlePointer
  :: Monad actionM
  => PointerEvent
  -> Handler actionM
  -> actionM ()
handlePointer event handler =
  runFirst event (reverse (pointerHandlers handler))

handleKey
  :: Monad actionM
  => KeyEvent
  -> Handler actionM
  -> actionM ()
handleKey event handler =
  runFirst event (keyHandlers handler)

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
