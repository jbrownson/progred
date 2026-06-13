module Puri.Halay
  ( halayWidget
  , lineEdit
  ) where

import Halay
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widget
import qualified Puri.Widgets.LineEdit as LineEdit
import Puri.Widgets.LineEdit (LineEditState, LineStyle)

halayWidget
  :: Applicative measureM
  => measureM Size
  -> Widget props actionM placeM
  -> props
  -> Halay measureM placeM (Handler actionM)
halayWidget measure widget props =
  leaf measure (renderWidget widget props)

lineEdit
  :: Canvas.Canvas renderM
  => LineStyle
  -> String
  -> Maybe LineEditState
  -> (String -> Maybe LineEditState -> actionM ())
  -> Halay renderM renderM (Handler actionM)
lineEdit style string state change =
  halayWidget
    (LineEdit.lineEditSize style string)
    (LineEdit.lineEdit style)
    (LineEdit.LineEditProps string state change)
