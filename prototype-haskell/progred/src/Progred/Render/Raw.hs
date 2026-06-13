module Progred.Render.Raw
  ( focusedProjection
  , rawProjection
  , textPlay
  ) where

import qualified Data.Map.Strict as Map
import Halay
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.Projection
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas
import Puri.Halay (lineEdit)
import Puri.Handler
import Puri.Widgets (LineEditInteraction (..), LineStyle (..))
import Puri.Widgets.Frame

-- The total projection at the bottom of every composition: assumes
-- nothing, renders whatever the spot holds, placeholders included.
rawProjection :: Canvas.Canvas renderM => Projection actionM renderM
rawProjection env cursor =
  case resolveCursor env cursor of
    Nothing -> textPlay missingColor "<missing>"
    Just resolved -> rawValue env resolved

focusedProjection :: Canvas.Canvas renderM => Projection actionM renderM -> Projection actionM renderM
focusedProjection projection env cursor =
  focusCursor cursor (projection env cursor)

focusCursor :: Canvas.Canvas renderM => Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
focusCursor cursor child =
  case cursorFocus cursor of
    Just focus | null (focusPath focus) ->
      decorate drawFocusBackground child
    _ -> child

drawFocusBackground :: Canvas.Canvas renderM => Rect -> renderM (Handler actionM)
drawFocusBackground rect = do
  Canvas.fillRect focusRect focusBackgroundColor
  Canvas.strokeRect focusRect focusColor 1
  pure mempty
  where
    focusRect = inflateRect 3 2 rect

inflateRect :: Double -> Double -> Rect -> Rect
inflateRect dx dy Rect {x, y, width, height} =
  Rect (x - dx) (y - dy) (width + dx * 2) (height + dy * 2)

rawValue :: Canvas.Canvas renderM => Env actionM renderM -> ResolvedCursor -> Halay renderM renderM (Handler actionM)
rawValue env resolved =
  case resolvedValue resolved of
    VRef target
      | target `elem` resolvedNodes resolved -> rowWithGap valueGap [identiconPlay target, textPlay repeatColor "..."]
      | otherwise -> rawNode env (resolvedCursor resolved) target
    VString string -> stringBox env cursor string
    VInt integer -> textPlay numberColor (show integer)
    VFloat double -> textPlay numberColor (show double)
    VBool bool -> textPlay boolColor (if bool then "true" else "false")
  where
    cursor = resolvedCursor resolved

rawNode :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> Halay renderM renderM (Handler actionM)
rawNode env cursor target =
  case lookupNode (envContext env) target of
    Nothing -> rowWithGap valueGap [identiconPlay target, textPlay missingColor "<missing>"]
    Just edges ->
      column
        [ identiconPlay target
        , box rawIndentBox [column (rawEdge <$> Map.toList edges)]
        ]
  where
    rawEdge (label, _value) =
      edgeRow env cursor label

edgeRow
  :: Canvas.Canvas renderM
  => Env actionM renderM
  -> Cursor
  -> UUID
  -> Halay renderM renderM (Handler actionM)
edgeRow env cursor label =
  decorate place $
    rowWithGap valueGap [rawEdgeLabel label, descend env cursor label]
  where
    path = cursorPath (descendCursor label cursor)
    place rect = do
      pure $
        onPointer $ \event ->
          case event of
            PointerDown {pointerX, pointerY}
              | rectContains rect pointerX pointerY ->
                  Just (envEdit env (focusEdge path))
            _ -> Nothing

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
rawEdgeLabel label =
  rowWithGap arrowGap [identiconPlay label, arrowPlay]

stringBox :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> String -> Halay renderM renderM (Handler actionM)
stringBox env cursor string =
  framed (stringFrame (isLineEditFocused interaction)) (lineEdit stringLineStyle string interaction)
  where
    path = cursorPath cursor
    interaction =
      case cursorFocus cursor of
        Just focus | null (focusPath focus) ->
          LineEditFocused
            (focusStringSelection (focusState focus))
            (\newString newSelection -> envEdit env (editString path newString newSelection))
            (envEdit env (blurString path))
        _ ->
          LineEditUnfocused (\selection -> envEdit env (focusString path selection))

isLineEditFocused :: LineEditInteraction actionM -> Bool
isLineEditFocused interaction =
  case interaction of
    LineEditUnfocused _focus -> False
    LineEditFocused _selection _change _blur -> True

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

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
identiconPlay uuid =
  leaf (pure (Size iconSize rowHeight)) draw
  where
    draw Rect {x, y} =
      mempty <$ identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM renderM (Handler actionM)
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

arrowPlay :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
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

focusBackgroundColor :: String
focusBackgroundColor = "#eaf3ff"

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
