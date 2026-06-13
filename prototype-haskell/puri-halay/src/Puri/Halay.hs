module Puri.Halay
  ( halayWidget
  , lineEdit
  ) where

import Halay
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widget
import qualified Puri.Widgets as Widgets
import Puri.Widgets (LineEditSelection, LineStyle)

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
  -> Maybe LineEditSelection
  -> (String -> Maybe LineEditSelection -> actionM ())
  -> Halay renderM renderM (Handler actionM)
lineEdit style string selection change =
  halayWidget
    (Widgets.lineEditSize edit)
    Widgets.lineEdit
    edit
  where
    edit =
      Widgets.LineEdit
        { Widgets.lineEditStyle = style
        , Widgets.lineEditText = string
        , Widgets.lineEditSelection = selection
        , Widgets.lineEditChange = change
        }
