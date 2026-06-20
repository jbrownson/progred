module Puri.Handler
  ( Handler (..)
  , KeyEvent (..)
  , KeyHandler
  , KeyModifiers (..)
  , PointerEvent (..)
  , PointerHandler
  , WheelEvent (..)
  , WheelHandler
  , handleDelete
  , handleInsert
  , handleKey
  , handlePointer
  , handleWheel
  , hasModifier
  , onDelete
  , onInsert
  , onKey
  , onPointer
  , onPointerCapture
  , onWheel
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

data WheelEvent = Wheel
  { wheelX :: Double
  , wheelY :: Double
  , wheelDeltaX :: Double
  , wheelDeltaY :: Double
  , wheelDeltaMode :: Word32
  , wheelModifiers :: KeyModifiers
  }

data KeyEvent
  = KeyCode KeyModifiers Word32
  | TextInput String

hasModifier :: KeyModifiers -> Bool
hasModifier modifiers =
  keyShift modifiers || keyAlt modifiers || keyCtrl modifiers || keyMeta modifiers

type PointerHandler actionM = PointerEvent -> Maybe (actionM ())

type WheelHandler actionM = WheelEvent -> Maybe (actionM ())

type KeyHandler actionM = KeyEvent -> Maybe (actionM ())

data Handler actionM = Handler
  { pointerCaptureHandler :: PointerHandler actionM
  , pointerHandler :: PointerHandler actionM
  , wheelHandler :: WheelHandler actionM
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
      , wheelHandler = firstClaim (wheelHandler later) (wheelHandler earlier)
      , keyHandler = firstClaim (keyHandler later) (keyHandler earlier)
      , deleteHandler = deleteHandler later <|> deleteHandler earlier
      , insertHandler = insertHandler later <|> insertHandler earlier
      }

instance Monoid (Handler actionM) where
  mempty =
    Handler (const Nothing) (const Nothing) (const Nothing) (const Nothing) Nothing Nothing

firstClaim :: (event -> Maybe action) -> (event -> Maybe action) -> event -> Maybe action
firstClaim first second event =
  first event <|> second event

onPointer :: PointerHandler actionM -> Handler actionM
onPointer handler =
  mempty {pointerHandler = handler}

onPointerCapture :: PointerHandler actionM -> Handler actionM
onPointerCapture handler =
  mempty {pointerCaptureHandler = handler}

onWheel :: WheelHandler actionM -> Handler actionM
onWheel handler =
  mempty {wheelHandler = handler}

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

handleWheel
  :: Applicative actionM
  => WheelEvent
  -> Handler actionM
  -> actionM ()
handleWheel event handler =
  fromMaybe (pure ()) (wheelHandler handler event)

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
