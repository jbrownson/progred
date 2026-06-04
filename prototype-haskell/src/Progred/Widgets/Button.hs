module Progred.Widgets.Button
  ( ButtonParams (..)
  , button
  ) where

import Progred.Frame
import Progred.Geometry
import Progred.Widget

data ButtonParams appM widgetM = ButtonParams
  { buttonActivate :: appM ()
  , buttonContent :: Frame widgetM
  }

button
  :: (Applicative widgetM, WidgetActions () appM widgetM)
  => ButtonParams appM widgetM
  -> Widget () widgetM
button params _ rect focus _onChange =
  mconcat
    [ buttonContent params
    , onPointer $ \event ->
        case event of
          PointerDown {pointerX, pointerY} ->
            if rectContains rect pointerX pointerY
              then Just (focusSelf *> liftApp (buttonActivate params))
              else Nothing
          _ -> Nothing
    , onKey $ \event ->
        case focus of
          WidgetFocused -> case event of
            KeyCode 13 -> Just (liftApp (buttonActivate params))
            KeyCode 32 -> Just (liftApp (buttonActivate params))
            _ -> Nothing
          WidgetUnfocused -> Nothing
    ]
