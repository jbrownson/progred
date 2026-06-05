module Puri.Widget
  ( Widget
  , WidgetActions (..)
  , WidgetFocus (..)
  , widgetIsFocused
  ) where

import Puri.Handler
import Puri.Geometry

type Widget state actionM renderM =
  state
    -> Rect
    -> WidgetFocus
    -> WidgetActions state actionM
    -> renderM (Handler actionM)

data WidgetActions state actionM = WidgetActions
  { widgetFocusSelf :: actionM ()
  , widgetSetState :: state -> actionM ()
  }

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused

widgetIsFocused :: WidgetFocus -> Bool
widgetIsFocused WidgetFocused =
  True
widgetIsFocused WidgetUnfocused =
  False
