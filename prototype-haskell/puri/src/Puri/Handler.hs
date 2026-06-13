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

import Control.Applicative ((<|>))
import Data.Maybe (fromMaybe)
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
  { pointerHandler :: PointerHandler actionM
  , keyHandler :: KeyHandler actionM
  }

-- Composition tries the later-combined handler first. Containers can combine
-- their own handler before child handlers so the deepest/topmost handler wins;
-- unclaimed events fall outward.
instance Semigroup (Handler actionM) where
  earlier <> later =
    Handler
      { pointerHandler = firstClaim (pointerHandler later) (pointerHandler earlier)
      , keyHandler = firstClaim (keyHandler later) (keyHandler earlier)
      }

instance Monoid (Handler actionM) where
  mempty = Handler (const Nothing) (const Nothing)

firstClaim :: (event -> Maybe action) -> (event -> Maybe action) -> event -> Maybe action
firstClaim first second event =
  first event <|> second event

onPointer :: PointerHandler actionM -> Handler actionM
onPointer handler =
  mempty {pointerHandler = handler}

onKey :: KeyHandler actionM -> Handler actionM
onKey handler =
  mempty {keyHandler = handler}

handlePointer
  :: Applicative actionM
  => PointerEvent
  -> Handler actionM
  -> actionM ()
handlePointer event handler =
  fromMaybe (pure ()) (pointerHandler handler event)

handleKey
  :: Applicative actionM
  => KeyEvent
  -> Handler actionM
  -> actionM ()
handleKey event handler =
  fromMaybe (pure ()) (keyHandler handler event)
