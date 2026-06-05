module Progred.Widget
  ( Widget
  , WidgetActions (..)
  , WidgetFocus (..)
  ) where

import Progred.Frame
import Progred.Geometry

type Widget state m =
  state
    -> Rect
    -> WidgetFocus
    -> WidgetActions state m
    -> Frame m

data WidgetActions state m = WidgetActions
  { widgetFocusSelf :: m ()
  , widgetSetState :: state -> m ()
  }

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused
