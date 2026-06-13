module Puri.Widget
  ( Widget (..)
  , WidgetFocus (..)
  , renderWidget
  , widgetIsFocused
  ) where

import Puri.Handler
import Puri.Geometry

newtype Widget props actionM renderM = Widget
  { runWidget :: props -> Rect -> renderM (Handler actionM)
  }

renderWidget :: Widget props actionM renderM -> props -> Rect -> renderM (Handler actionM)
renderWidget =
  runWidget

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused

widgetIsFocused :: WidgetFocus -> Bool
widgetIsFocused WidgetFocused =
  True
widgetIsFocused WidgetUnfocused =
  False
