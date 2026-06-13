module Puri.Widget
  ( Widget (..)
  , renderWidget
  ) where

import Puri.Handler
import Puri.Geometry

newtype Widget props actionM renderM = Widget
  { runWidget :: props -> Rect -> renderM (Handler actionM)
  }

renderWidget :: Widget props actionM renderM -> props -> Rect -> renderM (Handler actionM)
renderWidget =
  runWidget
