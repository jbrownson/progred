module Progred.Widget
  ( Widget
  , WidgetActions (..)
  , WidgetFocus (..)
  ) where

import Progred.Frame
import Progred.Geometry

type Widget state actionM renderM =
  state
    -> Rect
    -> WidgetFocus
    -> WidgetActions state actionM
    -> Frame actionM renderM

data WidgetActions state actionM = WidgetActions
  { widgetFocusSelf :: actionM ()
  , widgetSetState :: state -> actionM ()
  }

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused
