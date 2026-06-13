module Puri.Widget
  ( Widget (..)
  , renderWidget
  ) where

import Puri.Handler
import Puri.Geometry

newtype Widget actionM renderM = Widget
  { runWidget :: Rect -> renderM (Handler actionM)
  }

renderWidget :: Widget actionM renderM -> Rect -> renderM (Handler actionM)
renderWidget =
  runWidget
