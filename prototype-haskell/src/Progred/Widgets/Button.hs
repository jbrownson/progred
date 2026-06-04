module Progred.Widgets.Button
  ( ButtonParams (..)
  , button
  ) where

import Progred.Frame
import Progred.Geometry
import Progred.Widget

data ButtonParams actionM widgetM = ButtonParams
  { buttonActivate :: actionM ()
  , buttonContent :: Frame widgetM
  }

button
  :: (Applicative widgetM, WidgetActions () actionM widgetM)
  => ButtonParams actionM widgetM
  -> Widget () widgetM
button params _ rect focus _onChange =
  mconcat
    [ buttonContent params
    , onPointer $ \event ->
        case event of
          PointerDown {pointerX, pointerY} ->
            if rectContains rect pointerX pointerY
              then Just (focusSelf *> liftAction (buttonActivate params))
              else Nothing
          _ -> Nothing
    , onKey $ \event ->
        case focus of
          WidgetFocused -> case event of
            KeyCode 13 -> Just (liftAction (buttonActivate params))
            KeyCode 32 -> Just (liftAction (buttonActivate params))
            _ -> Nothing
          WidgetUnfocused -> Nothing
    ]
