module Progred.Widget
  ( Widget
  , WidgetActions (..)
  , WidgetFocus (..)
  ) where

import Progred.Handler
import Progred.Geometry

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
