module Progred.Render.Raw
  ( focusedProjection
  , inlineRowWithGap
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
import Puri.Widgets (LineEditInteraction (..), LineEditSelection (..), LineStyle (..))
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
  focusCursor env cursor (projection env cursor)

focusCursor :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
focusCursor env cursor child =
  case cursorFocus cursor of
    Just focus | null (focusPath focus) && shouldDrawFocusBackground env cursor ->
      decorate drawFocusBackground child
    _ -> child

shouldDrawFocusBackground :: Env actionM renderM -> Cursor -> Bool
shouldDrawFocusBackground env cursor =
  case resolvedValue <$> resolveCursor env cursor of
    Just (VString _) -> False
    Just (VInt _) -> False
    Just (VFloat _) -> False
    _ -> True

drawFocusBackground :: Canvas.Canvas renderM => Rect -> renderM (Handler actionM)
drawFocusBackground rect = do
  Canvas.fillRect rect focusBackgroundColor
  Canvas.strokeRect rect focusColor 1
  pure mempty

rawValue :: Canvas.Canvas renderM => Env actionM renderM -> ResolvedCursor -> Halay renderM renderM (Handler actionM)
rawValue env resolved =
  case resolvedValue resolved of
    VRef target
      | target `elem` resolvedNodes resolved -> inlineRowWithGap valueGap [identiconPlay target, textPlay repeatColor "..."]
      | otherwise -> rawNode env (resolvedCursor resolved) target
    VString string -> stringBox env cursor string
    VInt integer -> numberBox env cursor (show integer) parseIntValue editInt
    VFloat double -> numberBox env cursor (show double) parseFloatValue editFloat
    VBool bool -> textPlay boolColor (if bool then "true" else "false")
  where
    cursor = resolvedCursor resolved

rawNode :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> Halay renderM renderM (Handler actionM)
rawNode env cursor target =
  case lookupNode (envContext env) target of
    Nothing -> inlineRowWithGap valueGap [identiconPlay target, textPlay missingColor "<missing>"]
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
  focusableEdge env childCursor $
    inlineRowWithGap valueGap [rawEdgeLabel label, envProject env childCursor]
  where
    childCursor = descendCursor label cursor

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
rawEdgeLabel label =
  inlineRowWithGap arrowGap [identiconPlay label, arrowPlay]

inlineRowWithGap :: Applicative measureM => Double -> [Halay measureM placeM placed] -> Halay measureM placeM placed
inlineRowWithGap gap =
  box defaultBox {boxDirection = LeftToRight, boxGap = gap, boxCrossAlign = CrossCenter}

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

numberBox
  :: Canvas.Canvas renderM
  => Env actionM renderM
  -> Cursor
  -> String
  -> (String -> Maybe Value)
  -> ([UUID] -> String -> LineEditSelection -> Editor -> Editor)
  -> Halay renderM renderM (Handler actionM)
numberBox env cursor string parse change =
  framed (stringFrame focused) (lineEdit (numberLineStyle (isValidNumber editText)) editText interaction)
  where
    path = cursorPath cursor
    isValidNumber candidate =
      case parse candidate of
        Just _ -> True
        Nothing -> False
    (focused, editText, interaction) =
      case cursorFocus cursor of
        Just focus | null (focusPath focus) ->
          let edit = numberEditOrDefault string (focusState focus)
           in ( True
              , numberEditText edit
              , LineEditFocused
                  (numberEditSelection edit)
                  (\newString newSelection -> envEdit env (change path newString newSelection))
                  (envEdit env (blurValue path))
              )
        _ ->
          ( False
          , string
          , LineEditUnfocused (\selection -> envEdit env (focusNumber path string selection))
          )

numberEditOrDefault :: String -> FocusState -> NumberEdit
numberEditOrDefault string state =
  case focusNumberEdit state of
    Just edit -> edit
    Nothing -> NumberEdit string (LineEditSelection (length string) (length string) False)

isLineEditFocused :: LineEditInteraction actionM -> Bool
isLineEditFocused interaction =
  case interaction of
    LineEditUnfocused _focus -> False
    LineEditFocused _selection _change _blur -> True

stringFrame :: Bool -> Frame
stringFrame focused =
  Frame
    { framePadding = Insets 0 0 0 0
    , frameInsets = Insets 0 0 0 0
    , frameBackground = if focused then Just focusBackgroundColor else Nothing
    , frameColor = if focused then focusColor else boxBorderColor
    }

stringLineStyle :: LineStyle
stringLineStyle =
  LineStyle
    { lineVerticalPadding = scalarVerticalPadding
    , linePadding = boxPad
    , lineMinWidth = minBoxTextWidth
    , lineTextColor = stringColor
    , lineCaretColor = focusColor
    , lineSelectionColor = selectionColor
    }

numberLineStyle :: Bool -> LineStyle
numberLineStyle valid =
  stringLineStyle
    { lineTextColor = if valid then numberColor else invalidNumberColor
    }

rawIndentBox :: BoxConfig
rawIndentBox =
  defaultBox
    { boxDirection = TopToBottom
    , boxPadding = Insets 0 0 0 indent
    }

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
identiconPlay uuid =
  leaf (pure (Size iconSize iconSize)) draw
  where
    draw Rect {x, y} =
      mempty <$ identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM renderM (Handler actionM)
textPlay color string =
  text config string
  where
    config =
      TextConfig
        { textLineHeight = Nothing
        , textWrapMode = TextWrapWords
        , textAlign = TextAlignStart
        , textMeasure = measureTextLine
        , textPlaceLine = \_lineIndex line Rect {x, y} -> do
            metrics <- Canvas.measureText textMetricSample
            mempty <$ Canvas.fillText (Point x (y + Canvas.textFontBoundingBoxAscent metrics)) color line
        }
    measureTextLine line = do
      textMetrics <- Canvas.measureText line
      lineMetrics <- Canvas.measureText textMetricSample
      pure (Size (Canvas.textWidth textMetrics) (textMetricHeight lineMetrics))

arrowPlay :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
arrowPlay =
  leaf (pure (Size arrowWidth iconSize)) draw
  where
    draw Rect {x, y} =
      mempty <$ drawArrow (Point x (y + iconSize / 2))

iconSize :: Double
iconSize = 20

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

invalidNumberColor :: String
invalidNumberColor = "#b42318"

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

scalarVerticalPadding :: Double
scalarVerticalPadding = 2

textMetricSample :: String
textMetricSample = "Mg"

textMetricHeight :: Canvas.TextMetrics -> Double
textMetricHeight metrics =
  Canvas.textFontBoundingBoxAscent metrics + Canvas.textFontBoundingBoxDescent metrics

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
