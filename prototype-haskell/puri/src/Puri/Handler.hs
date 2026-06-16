module Puri.Handler
  ( Handler (..)
  , KeyEvent (..)
  , KeyHandler
  , KeyModifiers (..)
  , PointerEvent (..)
  , PointerHandler
  , handleDelete
  , handleInsert
  , handleKey
  , handlePointer
  , onDelete
  , onInsert
  , onKey
  , onPointer
  , onPointerCapture
  ) where

import Control.Applicative ((<|>))
import Data.Maybe (fromMaybe)
import Data.Word (Word32)

data PointerEvent
  = PointerDown
  { pointerX :: Double
  , pointerY :: Double
  , pointerModifiers :: KeyModifiers
  }
  | PointerMove
  { pointerX :: Double
  , pointerY :: Double
  , pointerModifiers :: KeyModifiers
  }
  | PointerUp
  { pointerX :: Double
  , pointerY :: Double
  , pointerModifiers :: KeyModifiers
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
  { pointerCaptureHandler :: PointerHandler actionM
  , pointerHandler :: PointerHandler actionM
  , keyHandler :: KeyHandler actionM
  , deleteHandler :: Maybe (actionM ())
  , insertHandler :: Maybe (actionM ())
  }

-- Composition tries the later-combined handler first. Containers can combine
-- their own handler before child handlers so the deepest/topmost handler wins;
-- unclaimed events fall outward.
instance Semigroup (Handler actionM) where
  earlier <> later =
    Handler
      { pointerCaptureHandler = firstClaim (pointerCaptureHandler later) (pointerCaptureHandler earlier)
      , pointerHandler = firstClaim (pointerHandler later) (pointerHandler earlier)
      , keyHandler = firstClaim (keyHandler later) (keyHandler earlier)
      , deleteHandler = deleteHandler later <|> deleteHandler earlier
      , insertHandler = insertHandler later <|> insertHandler earlier
      }

instance Monoid (Handler actionM) where
  mempty = Handler (const Nothing) (const Nothing) (const Nothing) Nothing Nothing

firstClaim :: (event -> Maybe action) -> (event -> Maybe action) -> event -> Maybe action
firstClaim first second event =
  first event <|> second event

onPointer :: PointerHandler actionM -> Handler actionM
onPointer handler =
  mempty {pointerHandler = handler}

onPointerCapture :: PointerHandler actionM -> Handler actionM
onPointerCapture handler =
  mempty {pointerCaptureHandler = handler}

onKey :: KeyHandler actionM -> Handler actionM
onKey handler =
  mempty {keyHandler = handler}

onDelete :: actionM () -> Handler actionM
onDelete action =
  mempty {deleteHandler = Just action}

onInsert :: actionM () -> Handler actionM
onInsert action =
  mempty {insertHandler = Just action}

handlePointer
  :: Applicative actionM
  => PointerEvent
  -> Handler actionM
  -> actionM ()
handlePointer event handler =
  fromMaybe (pure ()) (pointerCaptureHandler handler event <|> pointerHandler handler event)

handleKey
  :: Applicative actionM
  => KeyEvent
  -> Handler actionM
  -> actionM ()
handleKey event handler =
  fromMaybe (pure ()) (keyHandler handler event)

handleDelete
  :: Applicative actionM
  => Handler actionM
  -> actionM ()
handleDelete handler =
  fromMaybe (pure ()) (deleteHandler handler)

handleInsert
  :: Applicative actionM
  => Handler actionM
  -> actionM ()
handleInsert handler =
  fromMaybe (pure ()) (insertHandler handler)
