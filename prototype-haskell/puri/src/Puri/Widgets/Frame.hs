module Puri.Widgets.Frame
  ( Frame (..)
  , framed
  ) where

import Halay
import qualified Puri.Canvas as Canvas

-- Pure chrome around a child: layout padding plus a border stroked on the
-- padded rect shrunk by frameInsets. No behavior, no hit testing.
data Frame = Frame
  { framePadding :: Insets
  , frameInsets :: Insets
  , frameColor :: String
  }

framed :: (Canvas.Canvas renderM, Monoid placed) => Frame -> Halay renderM placed -> Halay renderM placed
framed frame child =
  decorate drawBorder (padding (framePadding frame) child)
  where
    drawBorder rect =
      mempty <$ Canvas.strokeRect (insetRect (frameInsets frame) rect) (frameColor frame) 1
