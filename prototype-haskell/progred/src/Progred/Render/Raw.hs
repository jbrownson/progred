module Progred.Render.Raw
  ( rawProjection
  , textPlay
  ) where

import qualified Data.Map.Strict as Map
import Data.Maybe (isJust)
import Halay
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.Projection
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widgets.Frame
import Puri.Widgets.LineEdit

-- The total projection at the bottom of every composition: assumes
-- nothing, renders whatever the spot holds, placeholders included.
rawProjection :: Canvas.Canvas renderM => TotalProjection actionM renderM
rawProjection env cursor =
  case walkPath (envDocument env) (cursorPath cursor) of
    Nothing -> textPlay missingColor "<missing>"
    Just (nodes, value) -> rawValue env cursor nodes value

rawValue :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> [UUID] -> Value -> Halay renderM (Handler actionM)
rawValue env cursor nodes value =
  case value of
    VRef target
      | target `elem` nodes -> rowWithGap valueGap [identiconPlay target, textPlay repeatColor "..."]
      | otherwise -> rawNode env cursor target
    VString string -> stringBox env cursor string
    VInt integer -> textPlay numberColor (show integer)
    VFloat double -> textPlay numberColor (show double)
    VBool bool -> textPlay boolColor (if bool then "true" else "false")

rawNode :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> Halay renderM (Handler actionM)
rawNode env cursor target =
  case Map.lookup target (documentGraph (envDocument env)) of
    Nothing -> rowWithGap valueGap [identiconPlay target, textPlay missingColor "<missing>"]
    Just edges ->
      column
        [ identiconPlay target
        , box rawIndentBox [column (rawEdge <$> Map.toList edges)]
        ]
  where
    rawEdge (label, _value) =
      rowWithGap valueGap [rawEdgeLabel label, envProject env (stepCursor label cursor)]

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM (Handler actionM)
rawEdgeLabel label =
  rowWithGap arrowGap [identiconPlay label, arrowPlay]

stringBox :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> String -> Halay renderM (Handler actionM)
stringBox env cursor string =
  framed (stringFrame (isJust view)) (lineEdit stringLineStyle string view change)
  where
    view =
      case cursorFocus cursor of
        Just (Focus [] editView) -> Just editView
        _ -> Nothing
    change newString newView =
      envEdit env (editString (cursorPath cursor) newString newView)

stringFrame :: Bool -> Frame
stringFrame focused =
  Frame
    { framePadding = Insets 0 0 0 0
    , frameInsets = Insets 0 0 boxBottomGap 0
    , frameColor = if focused then focusColor else boxBorderColor
    }

stringLineStyle :: LineStyle
stringLineStyle =
  LineStyle
    { lineHeight = rowHeight
    , lineBaseline = textBaseline
    , lineAscent = textAscent
    , lineDescent = textDescent
    , linePadding = boxPad
    , lineMinWidth = minBoxTextWidth
    , lineTextColor = stringColor
    , lineCaretColor = focusColor
    , lineSelectionColor = selectionColor
    }

rawIndentBox :: BoxConfig
rawIndentBox =
  defaultBox
    { boxDirection = TopToBottom
    , boxPadding = Insets 0 0 0 indent
    }

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM (Handler actionM)
identiconPlay uuid =
  leaf (pure (Size iconSize rowHeight)) draw
  where
    draw Rect {x, y} =
      mempty <$ identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM (Handler actionM)
textPlay color string =
  text config string
  where
    config =
      TextConfig
        { textLineHeight = Just rowHeight
        , textWrapMode = TextWrapWords
        , textAlign = TextAlignStart
        , textMeasure = \line -> Size <$> Canvas.measureText line <*> pure rowHeight
        , textPlaceLine = \_lineIndex line Rect {x, y} ->
            mempty <$ Canvas.fillText (Point x (y + textBaseline)) color line
        }

arrowPlay :: Canvas.Canvas renderM => Halay renderM (Handler actionM)
arrowPlay =
  leaf (pure (Size arrowWidth rowHeight)) draw
  where
    draw Rect {x, y} =
      mempty <$ drawArrow (Point x (y + iconSize / 2))

iconSize :: Double
iconSize = 20

rowHeight :: Double
rowHeight = 26

textBaseline :: Double
textBaseline = 16

indent :: Double
indent = 28

drawArrow :: Canvas.Canvas renderM => Point -> renderM ()
drawArrow Point {pointX, pointY} = do
  Canvas.fillRect (Rect pointX pointY arrowStemWidth 1) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth) (pointY - 2) 1 5) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth + 1) (pointY - 1) 1 3) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth + 2) pointY 1 1) arrowColor

arrowColor :: String
arrowColor = "#68707c"

stringColor :: String
stringColor = "#20242a"

numberColor :: String
numberColor = "#365f9f"

boolColor :: String
boolColor = "#7a3fa0"

missingColor :: String
missingColor = "#9a2d2d"

repeatColor :: String
repeatColor = "#8a5a00"

focusColor :: String
focusColor = "#0a84ff"

boxBorderColor :: String
boxBorderColor = "#c8ccd2"

boxPad :: Double
boxPad = 5

boxBottomGap :: Double
boxBottomGap = 4

textAscent :: Double
textAscent = 12

textDescent :: Double
textDescent = 2

selectionColor :: String
selectionColor = "#cfe3ff"

minBoxTextWidth :: Double
minBoxTextWidth = 6

arrowStemWidth :: Double
arrowStemWidth = 10

arrowWidth :: Double
arrowWidth = 13

arrowGap :: Double
arrowGap = 6

valueGap :: Double
valueGap = 10
