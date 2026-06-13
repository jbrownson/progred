module Puri.Halay
  ( halayWidget
  , lineEdit
  ) where

import Halay
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widget
import qualified Puri.Widgets as Widgets
import Puri.Widgets (LineEditInteraction, LineStyle)

halayWidget
  :: Applicative measureM
  => measureM Size
  -> Widget actionM placeM
  -> Halay measureM placeM (Handler actionM)
halayWidget measure widget =
  leaf measure widget

lineEdit
  :: Canvas.Canvas renderM
  => LineStyle
  -> String
  -> LineEditInteraction actionM
  -> Halay renderM renderM (Handler actionM)
lineEdit style string interaction =
  halayWidget
    (Widgets.lineEditSize edit)
    (Widgets.lineEdit edit)
  where
    edit =
      Widgets.LineEdit
        { Widgets.lineEditStyle = style
        , Widgets.lineEditText = string
        , Widgets.lineEditInteraction = interaction
        }
