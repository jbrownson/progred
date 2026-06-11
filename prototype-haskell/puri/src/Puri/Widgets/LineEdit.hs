{-# LANGUAGE LambdaCase #-}

module Puri.Widgets.LineEdit
  ( EditView (..)
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

-- Caret, selection, and drag state of a line edit. The text itself stays
-- with the caller; the selection anchor sits at caret + offset.
data EditView = EditView
  { editCaret :: Int
  , editSelectionOffset :: Int
  , editDragging :: Bool
  }
  deriving (Show)

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
-- text and the view (Nothing to defocus). The caller owns where each
-- part lives.
lineEdit
  :: Canvas.Canvas renderM
  => LineStyle
  -> String
  -> Maybe EditView
  -> (String -> Maybe EditView -> actionM ())
  -> Halay renderM (Handler actionM)
lineEdit style string view change =
  leaf measure place
  where
    state = toEditState string view
    measure = do
      textWidth <- Canvas.measureText string
      pure (Size (max (lineMinWidth style) textWidth + 2 * linePadding style) (lineHeight style))
    place rect = do
      caretPositions <- measureCaretPositions string
      drawLine style (isJust view) state rect caretPositions
      pure (editHandler style state (isJust view) change rect caretPositions)

data EditState = EditState
  { editBefore :: String
  , editAfter :: String
  , editOffset :: Int
  , editDrag :: Bool
  }

toEditState :: String -> Maybe EditView -> EditState
toEditState string maybeView =
  EditState
    { editBefore = before
    , editAfter = after
    , editOffset = editSelectionOffset view
    , editDrag = editDragging view
    }
  where
    view = fromMaybe (EditView 0 0 False) maybeView
    (before, after) = splitAt (max 0 (min (length string) (editCaret view))) string

fromEditState :: EditState -> (String, EditView)
fromEditState state =
  ( editText state
  , EditView
      { editCaret = caretIndex state
      , editSelectionOffset = editOffset state
      , editDragging = editDrag state
      }
  )

editHandler
  :: LineStyle
  -> EditState
  -> Bool
  -> (String -> Maybe EditView -> actionM ())
  -> Rect
  -> [(Int, Double)]
  -> Handler actionM
editHandler style state focused change rect caretPositions =
  mconcat
    [ onPointer pointerDown
    , if editDrag state then onPointer dragEvents else mempty
    , if focused then onKey keyDown else mempty
    ]
  where
    textX = x rect + linePadding style
    caretAt pointerX = closestCaretIndex caretPositions (pointerX - textX)
    pointerDown = \case
      PointerDown {pointerX, pointerY}
        | rectContains rect pointerX pointerY ->
            Just (apply (startDragAt (caretAt pointerX) state))
      _ -> Nothing
    dragEvents = \case
      PointerMove {pointerX} -> Just (apply (continueDragAt (caretAt pointerX) state))
      PointerUp {} -> Just (apply state {editDrag = False})
      _ -> Nothing
    keyDown event =
      case event of
        KeyCode _modifiers code
          | code == KeyCode.enter -> Just (change (editText state) Nothing)
          | code == KeyCode.escape -> Just (change (editText state) Nothing)
        _ -> apply <$> keyState event state
    apply newState =
      change newText (Just newView)
      where
        (newText, newView) = fromEditState newState

keyState :: KeyEvent -> EditState -> Maybe EditState
keyState event state =
  case event of
    TextInput string -> Just (insertString string state)
    KeyCode modifiers code
      | code == KeyCode.space -> Just (insertString " " state)
      | code == KeyCode.backspace -> Just (deleteBackward state)
      | code == KeyCode.delete -> Just (deleteForward state)
      | code == KeyCode.left -> Just (moveCaret (keyShift modifiers) (-1) state)
      | code == KeyCode.right -> Just (moveCaret (keyShift modifiers) 1 state)
      | code == KeyCode.home -> Just (moveCaretStart state)
      | code == KeyCode.end -> Just (moveCaretEnd state)
    _ -> Nothing

startDragAt :: Int -> EditState -> EditState
startDragAt newCaret state =
  (setCaretIndex newCaret state)
    { editOffset = 0
    , editDrag = True
    }

continueDragAt :: Int -> EditState -> EditState
continueDragAt newCaret state =
  moved
    { editOffset = anchor - caretIndex moved
    }
  where
    anchor = caretIndex state + editOffset state
    moved = setCaretIndex newCaret state

insertString :: String -> EditState -> EditState
insertString string state =
  withoutSelection
    { editBefore = editBefore withoutSelection <> string
    , editDrag = False
    }
  where
    withoutSelection = deleteSelection state

deleteBackward :: EditState -> EditState
deleteBackward state
  | hasSelection state = deleteSelection state
  | null (editBefore state) =
      state {editOffset = 0, editDrag = False}
  | otherwise =
      state
        { editBefore = init (editBefore state)
        , editOffset = 0
        , editDrag = False
        }

deleteForward :: EditState -> EditState
deleteForward state
  | hasSelection state = deleteSelection state
  | otherwise =
      state
        { editAfter = drop 1 (editAfter state)
        , editOffset = 0
        , editDrag = False
        }

moveCaret :: Bool -> Int -> EditState -> EditState
moveCaret extending delta state =
  moved
    { editOffset = if extending then anchor - caretIndex moved else 0
    , editDrag = False
    }
  where
    anchor = caretIndex state + editOffset state
    moved
      | delta < 0 = moveCaretLeft state
      | delta > 0 = moveCaretRight state
      | otherwise = state

moveCaretLeft :: EditState -> EditState
moveCaretLeft state
  | null (editBefore state) = state
  | otherwise =
      state
        { editBefore = init (editBefore state)
        , editAfter = last (editBefore state) : editAfter state
        }

moveCaretRight :: EditState -> EditState
moveCaretRight state =
  case editAfter state of
    [] -> state
    moved : after ->
      state
        { editBefore = editBefore state <> [moved]
        , editAfter = after
        }

moveCaretStart :: EditState -> EditState
moveCaretStart state =
  state
    { editBefore = ""
    , editAfter = editText state
    , editOffset = 0
    , editDrag = False
    }

moveCaretEnd :: EditState -> EditState
moveCaretEnd state =
  state
    { editBefore = editText state
    , editAfter = ""
    , editOffset = 0
    , editDrag = False
    }

hasSelection :: EditState -> Bool
hasSelection state =
  case selectionText state of
    Nothing -> False
    Just _ -> True

deleteSelection :: EditState -> EditState
deleteSelection state
  | editOffset state > 0 =
      state
        { editAfter = drop (editOffset state) (editAfter state)
        , editOffset = 0
        , editDrag = False
        }
  | editOffset state < 0 =
      state
        { editBefore = keepUnselectedBefore state
        , editOffset = 0
        , editDrag = False
        }
  | otherwise =
      state {editDrag = False}

setCaretIndex :: Int -> EditState -> EditState
setCaretIndex index state =
  state
    { editBefore = before
    , editAfter = after
    }
  where
    clampedIndex = max 0 (min (length (editText state)) index)
    (before, after) = splitAt clampedIndex (editText state)

caretIndex :: EditState -> Int
caretIndex =
  length . editBefore

editText :: EditState -> String
editText state =
  editBefore state <> editAfter state

drawLine :: Canvas.Canvas renderM => LineStyle -> Bool -> EditState -> Rect -> [(Int, Double)] -> renderM ()
drawLine style focused state Rect {x, y} caretPositions = do
  when focused (drawSelection style state textX selectionTop selectionHeight caretPositions)
  Canvas.fillText (Point textX (y + lineBaseline style)) (lineTextColor style) (editText state)
  when focused drawCaret
  where
    textX = x + linePadding style
    selectionTop = y + lineBaseline style - lineAscent style
    selectionHeight = lineAscent style + lineDescent style
    drawCaret =
      Canvas.fillRect
        (Rect (textX + caretXAt caretPositions (caretIndex state)) selectionTop 1.5 selectionHeight)
        (lineCaretColor style)

drawSelection :: Canvas.Canvas renderM => LineStyle -> EditState -> Double -> Double -> Double -> [(Int, Double)] -> renderM ()
drawSelection style state textX selectionTop selectionHeight caretPositions =
  case selectionText state of
    Nothing -> pure ()
    Just (beforeSelection, selection) ->
      Canvas.fillRect
        (Rect (textX + beforeWidth) selectionTop selectionWidth selectionHeight)
        (lineSelectionColor style)
      where
        beforeIndex = length beforeSelection
        afterIndex = beforeIndex + length selection
        beforeWidth = caretXAt caretPositions beforeIndex
        selectionWidth = caretXAt caretPositions afterIndex - beforeWidth

selectionText :: EditState -> Maybe (String, String)
selectionText state
  | editOffset state > 0 =
      nonemptySelection (editBefore state) (take (editOffset state) (editAfter state))
  | editOffset state < 0 =
      nonemptySelection beforeSelection (drop (length beforeSelection) (editBefore state))
  | otherwise =
      Nothing
  where
    beforeSelection = keepUnselectedBefore state

nonemptySelection :: String -> String -> Maybe (String, String)
nonemptySelection beforeSelection selection
  | null selection = Nothing
  | otherwise = Just (beforeSelection, selection)

keepUnselectedBefore :: EditState -> String
keepUnselectedBefore state =
  take keepCount (editBefore state)
  where
    selectedCount = min (negate (editOffset state)) (length (editBefore state))
    keepCount = length (editBefore state) - selectedCount

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
