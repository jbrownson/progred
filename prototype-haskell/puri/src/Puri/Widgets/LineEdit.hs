module Puri.Widgets.LineEdit
  ( LineEdit (..)
  , LineEditFocus (..)
  , LineEditSelection (..)
  , LineStyle (..)
  , lineEdit
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

data LineEditFocus
  = LineEditUnfocused
  | LineEditFocused LineEditSelection
  deriving (Eq, Show)

data LineStyle = LineStyle
  { lineHeight :: Double
  , lineBaseline :: Double
  , lineAscent :: Double
  , lineDescent :: Double
  , linePadding :: Double
  , lineMinWidth :: Double
  , lineTextColor :: String
  , lineCaretColor :: String
  , lineSelectionColor :: String
  }

data LineEdit actionM = LineEdit
  { lineEditStyle :: LineStyle
  , lineEditText :: String
  , lineEditFocus :: LineEditFocus
  , lineEditChange :: String -> LineEditFocus -> actionM ()
  }

-- The change callback reports the widget's complete desired value: the
-- text and the focus-local selection. The caller owns where each part
-- lives.
lineEdit :: Canvas.Canvas renderM => LineEdit actionM -> Widget actionM renderM
lineEdit edit rect = do
  let style = lineEditStyle edit
  let string = lineEditText edit
  let focus = lineEditFocus edit
  let selection = focusSelection focus
  let focused = isFocused focus
  caretPositions <- measureCaretPositions string
  drawLine style focused string selection rect caretPositions
  pure (editHandler style string selection focused (lineEditChange edit) rect caretPositions)

lineEditSize :: Canvas.Canvas measureM => LineEdit actionM -> measureM Size
lineEditSize edit = do
  let style = lineEditStyle edit
  let string = lineEditText edit
  textWidth <- Canvas.measureText string
  pure (Size (max (lineMinWidth style) textWidth + 2 * linePadding style) (lineHeight style))

editHandler
  :: LineStyle
  -> String
  -> LineEditSelection
  -> Bool
  -> (String -> LineEditFocus -> actionM ())
  -> Rect
  -> [(Int, Double)]
  -> Handler actionM
editHandler style string selection focused change rect caretPositions =
  Handler
    { pointerHandler = pointer
    , keyHandler = if focused then key else const Nothing
    }
  where
    textX = x rect + linePadding style
    caretAt pointerX = closestCaretIndex caretPositions (pointerX - textX)
    pointer event =
      case event of
        PointerDown {pointerX, pointerY}
          | rectContains rect pointerX pointerY ->
              Just (change string (LineEditFocused (startDragAt (caretAt pointerX))))
        PointerMove {pointerX}
          | editDragging selection ->
              Just (change string (LineEditFocused (continueDragAt (caretAt pointerX) selection)))
        PointerUp {}
          | editDragging selection ->
              Just (change string (LineEditFocused (selection {editDragging = False})))
        _ -> Nothing
    key event =
      case event of
        KeyCode _modifiers code
          | code == KeyCode.enter || code == KeyCode.escape ->
              Just (change string LineEditUnfocused)
        _ -> report <$> keyEdit string event selection
    report (newString, newSelection) = change newString (LineEditFocused newSelection)

focusSelection :: LineEditFocus -> LineEditSelection
focusSelection focus =
  case focus of
    LineEditUnfocused -> collapsed 0
    LineEditFocused selection -> selection

isFocused :: LineEditFocus -> Bool
isFocused focus =
  case focus of
    LineEditUnfocused -> False
    LineEditFocused _selection -> True

keyEdit :: String -> KeyEvent -> LineEditSelection -> Maybe (String, LineEditSelection)
keyEdit string event selection =
  case event of
    TextInput inserted -> Just (insertString inserted string selection)
    KeyCode modifiers code
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

drawLine :: Canvas.Canvas renderM => LineStyle -> Bool -> String -> LineEditSelection -> Rect -> [(Int, Double)] -> renderM ()
drawLine style focused string selection Rect {x, y} caretPositions = do
  when (focused && lo /= hi) drawSelection
  Canvas.fillText (Point textX (y + lineBaseline style)) (lineTextColor style) string
  when focused drawCaret
  where
    textX = x + linePadding style
    selectionTop = y + lineBaseline style - lineAscent style
    selectionHeight = lineAscent style + lineDescent style
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
      prefixWidth <- Canvas.measureText (take index string)
      pure (index, prefixWidth)

closestCaretIndex :: [(Int, Double)] -> Double -> Int
closestCaretIndex caretPositions targetX =
  fst (minimumBy (comparing distanceFromTarget) caretPositions)
  where
    distanceFromTarget (_index, caretX) =
      abs (caretX - targetX)

caretXAt :: [(Int, Double)] -> Int -> Double
caretXAt caretPositions index =
  fromMaybe 0 (lookup index caretPositions)
