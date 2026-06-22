module Puri.Widgets.Frame
  ( Frame (..)
  , framed
  ) where

import Halay
import qualified Puri.Canvas as Canvas

-- Pure chrome around a child: layout padding plus optional background and a
-- border drawn on the padded rect shrunk by frameInsets. No behavior, no hit
-- testing.
data Frame = Frame
  { framePadding :: Insets
  , frameInsets :: Insets
  , frameBackground :: Maybe String
  , frameColor :: String
  }

framed :: (Applicative measureM, Canvas.Canvas placeM, Monoid placed) => Frame -> Halay measureM placeM placed -> Halay measureM placeM placed
framed frame child =
  decorate drawBorder (withBackground (padding (framePadding frame) child))
  where
    frameRect = insetRect (frameInsets frame)
    withBackground =
      case frameBackground frame of
        Nothing -> id
        Just color -> decorate (\placement -> mempty <$ Canvas.fillRect (frameRect (placementRect placement)) color)
    drawBorder placement =
      mempty <$ Canvas.strokeRect (frameRect (placementRect placement)) (frameColor frame) 1
