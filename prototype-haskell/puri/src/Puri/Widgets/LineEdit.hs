module Puri.Widgets.LineEdit
  ( LineEdit (..)
  , LineEditInteraction (..)
  , LineEditSelection (..)
  , LineStyle (..)
  , emptyLineEditSelection
  , lineEdit
  , lineEditSelectionAtEnd
  , lineEditSize
  ) where

import Control.Monad (when)
import Data.List (minimumBy)
import Data.Maybe (fromMaybe)
import Data.Ord (comparing)
import qualified Puri.Canvas as Canvas
import Puri.Geometry
import Puri.Handler
import qualified Puri.KeyCode as KeyCode
import Puri.Widget

-- Caret and selection of a line edit, as absolute indices into the
-- text. The text itself stays with the caller; the caret is the active
-- end of the selection and the anchor its other end (equal when there
-- is no selection). Indices are clamped against the text wherever read,
-- so they never need to stay in sync with it.
data LineEditSelection = LineEditSelection
  { editCaret :: Int
  , editAnchor :: Int
  , editDragging :: Bool
  }
  deriving (Eq, Show)

data LineEditInteraction actionM
  = LineEditUnfocused (LineEditSelection -> actionM ())
  | LineEditFocused LineEditSelection (String -> LineEditSelection -> actionM ()) (actionM ())

data LineStyle = LineStyle
  { lineVerticalPadding :: Double
  , linePadding :: Double
  , lineMinWidth :: Double
  , lineTextColor :: String
  , lineCaretColor :: String
  , lineSelectionColor :: String
  }

data LineEdit actionM = LineEdit
  { lineEditStyle :: LineStyle
  , lineEditText :: String
  , lineEditInteraction :: LineEditInteraction actionM
  }

emptyLineEditSelection :: LineEditSelection
emptyLineEditSelection =
  collapsed 0

lineEditSelectionAtEnd :: String -> LineEditSelection
lineEditSelectionAtEnd string =
  collapsed (length string)

-- Focused and unfocused line edits expose different callbacks: an
-- unfocused edit can only request focus, while a focused edit can
-- change text/selection or blur itself.
lineEdit :: Canvas.Canvas renderM => LineEdit actionM -> Widget actionM renderM
lineEdit edit placement = do
  let rect = placementRect placement
  let hitRect = clipRect placement
  let style = lineEditStyle edit
  let string = lineEditText edit
  let interaction = lineEditInteraction edit
  let selection = interactionSelection interaction
  let focused = interactionFocused interaction
  lineMetrics <- Canvas.measureText lineMetricSample
  caretPositions <- measureCaretPositions string
  drawLine style focused string selection rect lineMetrics caretPositions
  pure (editHandler style string interaction hitRect caretPositions)

lineEditSize :: Canvas.Canvas measureM => LineEdit actionM -> measureM Size
lineEditSize edit = do
  let style = lineEditStyle edit
  let string = lineEditText edit
  textMetrics <- Canvas.measureText string
  lineMetrics <- Canvas.measureText lineMetricSample
  pure (Size (max (lineMinWidth style) (Canvas.textWidth textMetrics) + 2 * linePadding style) (lineBoxHeight style lineMetrics))

editHandler
  :: LineStyle
  -> String
  -> LineEditInteraction actionM
  -> Rect
  -> [(Int, Double)]
  -> Handler actionM
editHandler style string interaction rect caretPositions =
  mempty
    { pointerHandler = pointer
    , keyHandler = key
    }
  where
    selection = interactionSelection interaction
    textX = x rect + linePadding style
    caretAt pointerX = closestCaretIndex caretPositions (pointerX - textX)
    pointer event =
      case event of
        PointerDown {pointerX, pointerY}
          | rectContains rect pointerX pointerY ->
              Just (focusAt (caretAt pointerX))
        PointerMove {pointerX}
          | LineEditFocused _selection change _blur <- interaction
          , editDragging selection ->
              Just (change string (continueDragAt (caretAt pointerX) selection))
        PointerUp {}
          | LineEditFocused _selection change _blur <- interaction
          , editDragging selection ->
              Just (change string (selection {editDragging = False}))
        _ -> Nothing
    key event =
      case interaction of
        LineEditUnfocused _focus -> Nothing
        LineEditFocused _selection change blur ->
          case event of
            KeyCode _modifiers code
              | code == KeyCode.enter ->
                  Just blur
            _ -> report change <$> keyEdit string event selection
    focusAt newCaret =
      case interaction of
        LineEditUnfocused focus -> focus (startDragAt newCaret)
        LineEditFocused _selection change _blur -> change string (startDragAt newCaret)
    report change (newString, newSelection) = change newString newSelection

interactionSelection :: LineEditInteraction actionM -> LineEditSelection
interactionSelection interaction =
  case interaction of
    LineEditUnfocused _focus -> collapsed 0
    LineEditFocused selection _change _blur -> selection

interactionFocused :: LineEditInteraction actionM -> Bool
interactionFocused interaction =
  case interaction of
    LineEditUnfocused _focus -> False
    LineEditFocused _selection _change _blur -> True

keyEdit :: String -> KeyEvent -> LineEditSelection -> Maybe (String, LineEditSelection)
keyEdit string event selection =
  case event of
    TextInput inserted -> Just (insertString inserted string selection)
    KeyCode modifiers code
      | code == KeyCode.comma -> Just (insertString "," string selection)
      | code == KeyCode.space -> Just (insertString " " string selection)
      | code == KeyCode.backspace -> Just (deleteBackward string selection)
      | code == KeyCode.delete -> Just (deleteForward string selection)
      | code == KeyCode.left -> Just (string, moveCaret (keyShift modifiers) (-1) string selection)
      | code == KeyCode.right -> Just (string, moveCaret (keyShift modifiers) 1 string selection)
      | code == KeyCode.home -> Just (string, collapsed 0)
      | code == KeyCode.end -> Just (string, collapsed (length string))
    _ -> Nothing

insertString :: String -> String -> LineEditSelection -> (String, LineEditSelection)
insertString inserted string selection =
  (take lo string <> inserted <> drop hi string, collapsed (lo + length inserted))
  where
    (lo, hi) = selectionBounds string selection

deleteBackward :: String -> LineEditSelection -> (String, LineEditSelection)
deleteBackward string selection
  | lo /= hi = (take lo string <> drop hi string, collapsed lo)
  | lo == 0 = (string, collapsed 0)
  | otherwise = (take (lo - 1) string <> drop lo string, collapsed (lo - 1))
  where
    (lo, hi) = selectionBounds string selection

deleteForward :: String -> LineEditSelection -> (String, LineEditSelection)
deleteForward string selection
  | lo /= hi = (take lo string <> drop hi string, collapsed lo)
  | otherwise = (take lo string <> drop (lo + 1) string, collapsed lo)
  where
    (lo, hi) = selectionBounds string selection

moveCaret :: Bool -> Int -> String -> LineEditSelection -> LineEditSelection
moveCaret extending delta string selection =
  LineEditSelection
    { editCaret = moved
    , editAnchor = if extending then clampIndex string (editAnchor selection) else moved
    , editDragging = False
    }
  where
    moved = clampIndex string (clampIndex string (editCaret selection) + delta)

startDragAt :: Int -> LineEditSelection
startDragAt index =
  LineEditSelection index index True

continueDragAt :: Int -> LineEditSelection -> LineEditSelection
continueDragAt index selection =
  selection {editCaret = index}

collapsed :: Int -> LineEditSelection
collapsed index =
  LineEditSelection index index False

selectionBounds :: String -> LineEditSelection -> (Int, Int)
selectionBounds string selection =
  (min caret anchor, max caret anchor)
  where
    caret = clampIndex string (editCaret selection)
    anchor = clampIndex string (editAnchor selection)

clampIndex :: String -> Int -> Int
clampIndex string index =
  max 0 (min (length string) index)

drawLine :: Canvas.Canvas renderM => LineStyle -> Bool -> String -> LineEditSelection -> Rect -> Canvas.TextMetrics -> [(Int, Double)] -> renderM ()
drawLine style focused string selection Rect {x, y} lineMetrics caretPositions = do
  when (focused && lo /= hi) drawSelection
  Canvas.fillText (Point textX (y + lineBaseline style lineMetrics)) (lineTextColor style) string
  when focused drawCaret
  where
    textX = x + linePadding style
    selectionTop = y + lineVerticalPadding style
    selectionHeight = lineFontHeight lineMetrics
    (lo, hi) = selectionBounds string selection
    caret = clampIndex string (editCaret selection)
    drawSelection =
      Canvas.fillRect
        (Rect (textX + caretXAt caretPositions lo) selectionTop (caretXAt caretPositions hi - caretXAt caretPositions lo) selectionHeight)
        (lineSelectionColor style)
    drawCaret =
      Canvas.fillRect
        (Rect (textX + caretXAt caretPositions caret) selectionTop 1.5 selectionHeight)
        (lineCaretColor style)

measureCaretPositions :: Canvas.Canvas renderM => String -> renderM [(Int, Double)]
measureCaretPositions string =
  traverse measureIndex [0 .. length string]
  where
    measureIndex index = do
      prefixMetrics <- Canvas.measureText (take index string)
      pure (index, Canvas.textWidth prefixMetrics)

lineMetricSample :: String
lineMetricSample = "Mg"

lineBaseline :: LineStyle -> Canvas.TextMetrics -> Double
lineBaseline style metrics =
  lineVerticalPadding style + Canvas.textFontBoundingBoxAscent metrics

lineBoxHeight :: LineStyle -> Canvas.TextMetrics -> Double
lineBoxHeight style metrics =
  lineFontHeight metrics + 2 * lineVerticalPadding style

lineFontHeight :: Canvas.TextMetrics -> Double
lineFontHeight metrics =
  Canvas.textFontBoundingBoxAscent metrics + Canvas.textFontBoundingBoxDescent metrics

closestCaretIndex :: [(Int, Double)] -> Double -> Int
closestCaretIndex caretPositions targetX =
  fst (minimumBy (comparing distanceFromTarget) caretPositions)
  where
    distanceFromTarget (_index, caretX) =
      abs (caretX - targetX)

caretXAt :: [(Int, Double)] -> Int -> Double
caretXAt caretPositions index =
  fromMaybe 0 (lookup index caretPositions)
