module Puri.Widgets.LineEdit
  ( LineEditState (..)
  , LineStyle (..)
  , lineEdit
  ) where

import Control.Monad (when)
import Data.List (minimumBy)
import Data.Maybe (fromMaybe, isJust)
import Data.Ord (comparing)
import Halay
import qualified Puri.Canvas as Canvas
import Puri.Handler
import qualified Puri.KeyCode as KeyCode

-- Caret and selection of a line edit, as absolute indices into the
-- text. The text itself stays with the caller; the caret is the active
-- end of the selection and the anchor its other end (equal when there
-- is no selection). Indices are clamped against the text wherever read,
-- so they never need to stay in sync with it.
data LineEditState = LineEditState
  { editCaret :: Int
  , editAnchor :: Int
  , editDragging :: Bool
  }
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

-- The change callback reports the widget's complete desired state: the
-- text and the state (Nothing to defocus). The caller owns where each
-- part lives.
lineEdit
  :: Canvas.Canvas renderM
  => LineStyle
  -> String
  -> Maybe LineEditState
  -> (String -> Maybe LineEditState -> actionM ())
  -> Halay renderM (Handler actionM)
lineEdit style string maybeState change =
  leaf measure place
  where
    state = fromMaybe (collapsed 0) maybeState
    measure = do
      textWidth <- Canvas.measureText string
      pure (Size (max (lineMinWidth style) textWidth + 2 * linePadding style) (lineHeight style))
    place rect = do
      caretPositions <- measureCaretPositions string
      drawLine style (isJust maybeState) string state rect caretPositions
      pure (editHandler style string state (isJust maybeState) change rect caretPositions)

editHandler
  :: LineStyle
  -> String
  -> LineEditState
  -> Bool
  -> (String -> Maybe LineEditState -> actionM ())
  -> Rect
  -> [(Int, Double)]
  -> Handler actionM
editHandler style string state focused change rect caretPositions =
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
              Just (change string (Just (startDragAt (caretAt pointerX))))
        PointerMove {pointerX}
          | editDragging state ->
              Just (change string (Just (continueDragAt (caretAt pointerX) state)))
        PointerUp {}
          | editDragging state ->
              Just (change string (Just state {editDragging = False}))
        _ -> Nothing
    key event =
      case event of
        KeyCode _modifiers code
          | code == KeyCode.enter || code == KeyCode.escape ->
              Just (change string Nothing)
        _ -> report <$> keyEdit string event state
    report (newString, newState) = change newString (Just newState)

keyEdit :: String -> KeyEvent -> LineEditState -> Maybe (String, LineEditState)
keyEdit string event state =
  case event of
    TextInput inserted -> Just (insertString inserted string state)
    KeyCode modifiers code
      | code == KeyCode.space -> Just (insertString " " string state)
      | code == KeyCode.backspace -> Just (deleteBackward string state)
      | code == KeyCode.delete -> Just (deleteForward string state)
      | code == KeyCode.left -> Just (string, moveCaret (keyShift modifiers) (-1) string state)
      | code == KeyCode.right -> Just (string, moveCaret (keyShift modifiers) 1 string state)
      | code == KeyCode.home -> Just (string, collapsed 0)
      | code == KeyCode.end -> Just (string, collapsed (length string))
    _ -> Nothing

insertString :: String -> String -> LineEditState -> (String, LineEditState)
insertString inserted string state =
  (take lo string <> inserted <> drop hi string, collapsed (lo + length inserted))
  where
    (lo, hi) = selectionBounds string state

deleteBackward :: String -> LineEditState -> (String, LineEditState)
deleteBackward string state
  | lo /= hi = (take lo string <> drop hi string, collapsed lo)
  | lo == 0 = (string, collapsed 0)
  | otherwise = (take (lo - 1) string <> drop lo string, collapsed (lo - 1))
  where
    (lo, hi) = selectionBounds string state

deleteForward :: String -> LineEditState -> (String, LineEditState)
deleteForward string state
  | lo /= hi = (take lo string <> drop hi string, collapsed lo)
  | otherwise = (take lo string <> drop (lo + 1) string, collapsed lo)
  where
    (lo, hi) = selectionBounds string state

moveCaret :: Bool -> Int -> String -> LineEditState -> LineEditState
moveCaret extending delta string state =
  LineEditState
    { editCaret = moved
    , editAnchor = if extending then clampIndex string (editAnchor state) else moved
    , editDragging = False
    }
  where
    moved = clampIndex string (clampIndex string (editCaret state) + delta)

startDragAt :: Int -> LineEditState
startDragAt index =
  LineEditState index index True

continueDragAt :: Int -> LineEditState -> LineEditState
continueDragAt index state =
  state {editCaret = index}

collapsed :: Int -> LineEditState
collapsed index =
  LineEditState index index False

selectionBounds :: String -> LineEditState -> (Int, Int)
selectionBounds string state =
  (min caret anchor, max caret anchor)
  where
    caret = clampIndex string (editCaret state)
    anchor = clampIndex string (editAnchor state)

clampIndex :: String -> Int -> Int
clampIndex string index =
  max 0 (min (length string) index)

drawLine :: Canvas.Canvas renderM => LineStyle -> Bool -> String -> LineEditState -> Rect -> [(Int, Double)] -> renderM ()
drawLine style focused string state Rect {x, y} caretPositions = do
  when (focused && lo /= hi) drawSelection
  Canvas.fillText (Point textX (y + lineBaseline style)) (lineTextColor style) string
  when focused drawCaret
  where
    textX = x + linePadding style
    selectionTop = y + lineBaseline style - lineAscent style
    selectionHeight = lineAscent style + lineDescent style
    (lo, hi) = selectionBounds string state
    caret = clampIndex string (editCaret state)
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
