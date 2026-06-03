module Progred.Widgets.TextBox
  ( TextBoxState (..)
  , initialTextBox
  , textBox
  ) where

import Control.Monad (when)
import Data.List (minimumBy)
import Data.Maybe (fromMaybe)
import Data.Ord (comparing)
import qualified Progred.Platform as Platform
import Progred.UI

data TextBoxState = TextBoxState
  { textBoxText :: String
  , textBoxCaret :: Int
  , textBoxSelectionAnchor :: Maybe Int
  , textBoxDragAnchor :: Maybe Int
  }

initialTextBox :: String -> TextBoxState
initialTextBox text =
  TextBoxState
    { textBoxText = text
    , textBoxCaret = length text
    , textBoxSelectionAnchor = Nothing
    , textBoxDragAnchor = Nothing
    }

textBox
  :: Eq focus
  => (world -> Maybe focus)
  -> (Maybe focus -> world -> world)
  -> world
  -> focus
  -> Rect
  -> (world -> TextBoxState)
  -> (TextBoxState -> world -> world)
  -> Render world IO ()
textBox getFocus setFocus world focusId rect get set =
  do
    drawMeasuredSelection rect value
    fillTextMiddle (Point textX textY) textColor (textBoxText value)
    when selected $
      drawMeasuredCaret rect value
    onPointer $ \current event ->
      case event of
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then
              Just $ do
                let old = get current
                let newCaret = caretFromX old pointerX
                let updated =
                      old
                        { textBoxCaret = newCaret
                        , textBoxSelectionAnchor = Nothing
                        , textBoxDragAnchor = Just newCaret
                        }
                pure (set updated (setFocus (Just focusId) current))
            else Nothing
        PointerMove {pointerX} ->
          case textBoxDragAnchor (get current) of
            Nothing -> Nothing
            Just anchor ->
              Just $ do
                let old = get current
                let newCaret = caretFromX old pointerX
                let updated =
                      old
                        { textBoxCaret = newCaret
                        , textBoxSelectionAnchor = Just anchor
                        }
                pure (set updated current)
        PointerUp {} ->
          case textBoxDragAnchor (get current) of
            Nothing -> Nothing
            Just _ ->
              Just $ do
                let old = get current
                pure (set old {textBoxDragAnchor = Nothing} current)
    onKey $ \current event ->
      if getFocus current == Just focusId
        then fmap pure (editText event current)
        else Nothing
  where
    value = get world
    selected = getFocus world == Just focusId
    textX = x rect
    textY = y rect + height rect / 2
    caretFromX old pointerX =
      closestCaretIndex (textBoxText old) (pointerX - textX)
    editText event current =
      let old = get current
       in case event of
            TextInput string -> Just (set (insertString string old) current)
            KeyCode 32 -> Just (set (insertString " " old) current)
            KeyCode 8 -> Just (set (deleteBackward old) current)
            KeyCode 46 -> Just (set (deleteForward old) current)
            KeyCode 37 -> Just (set (moveCaret False (-1) old) current)
            KeyCode 39 -> Just (set (moveCaret False 1 old) current)
            KeyCode 1037 -> Just (set (moveCaret True (-1) old) current)
            KeyCode 1039 -> Just (set (moveCaret True 1 old) current)
            KeyCode 36 -> Just (set old {textBoxCaret = 0, textBoxSelectionAnchor = Nothing} current)
            KeyCode 35 -> Just (set old {textBoxCaret = length (textBoxText old), textBoxSelectionAnchor = Nothing} current)
            _ -> Nothing

insertString :: String -> TextBoxState -> TextBoxState
insertString string textState =
  textState
    { textBoxText = before <> string <> after
    , textBoxCaret = start + length string
    , textBoxSelectionAnchor = Nothing
    , textBoxDragAnchor = Nothing
    }
  where
    (start, end) = editRange textState
    (before, selectedAndAfter) = splitAt start (textBoxText textState)
    after = drop (end - start) selectedAndAfter

deleteBackward :: TextBoxState -> TextBoxState
deleteBackward textState
  | hasSelection textState = replaceRange "" textState
  | textBoxCaret textState <= 0 =
      textState {textBoxSelectionAnchor = Nothing}
  | otherwise =
      textState
        { textBoxText = before <> after
        , textBoxCaret = textBoxCaret textState - 1
        , textBoxSelectionAnchor = Nothing
        }
  where
    (beforeWithDeleted, after) = splitAt (textBoxCaret textState) (textBoxText textState)
    before = take (length beforeWithDeleted - 1) beforeWithDeleted

deleteForward :: TextBoxState -> TextBoxState
deleteForward textState
  | hasSelection textState = replaceRange "" textState
  | otherwise =
      textState
        { textBoxText = before <> drop 1 after
        , textBoxSelectionAnchor = Nothing
        }
  where
    (before, after) = splitAt (textBoxCaret textState) (textBoxText textState)

moveCaret :: Bool -> Int -> TextBoxState -> TextBoxState
moveCaret extending delta textState =
  textState
    { textBoxCaret = newCaret
    , textBoxSelectionAnchor = if extending then Just anchor else Nothing
    , textBoxDragAnchor = Nothing
    }
  where
    oldCaret = textBoxCaret textState
    newCaret = max 0 (min (length (textBoxText textState)) (oldCaret + delta))
    anchor = fromMaybe oldCaret (textBoxSelectionAnchor textState)

hasSelection :: TextBoxState -> Bool
hasSelection textState =
  case selectedRange textState of
    Nothing -> False
    Just _ -> True

editRange :: TextBoxState -> (Int, Int)
editRange textState =
  case selectedRange textState of
    Nothing -> (textBoxCaret textState, textBoxCaret textState)
    Just (start, end) -> orderedRange start end

replaceRange :: String -> TextBoxState -> TextBoxState
replaceRange replacement textState =
  textState
    { textBoxText = before <> replacement <> after
    , textBoxCaret = start + length replacement
    , textBoxSelectionAnchor = Nothing
    , textBoxDragAnchor = Nothing
    }
  where
    (start, end) = editRange textState
    (before, selectedAndAfter) = splitAt start (textBoxText textState)
    after = drop (end - start) selectedAndAfter

selectedRange :: TextBoxState -> Maybe (Int, Int)
selectedRange TextBoxState {textBoxSelectionAnchor = Nothing} =
  Nothing
selectedRange TextBoxState {textBoxCaret, textBoxSelectionAnchor = Just anchor}
  | anchor == textBoxCaret = Nothing
  | otherwise = Just (orderedRange anchor textBoxCaret)

orderedRange :: Int -> Int -> (Int, Int)
orderedRange start end =
  (min start end, max start end)

drawMeasuredCaret :: Rect -> TextBoxState -> Render world IO ()
drawMeasuredCaret rect TextBoxState {textBoxText, textBoxCaret} =
  fillRect (Rect (x rect + prefixWidth) (y rect) 1 (height rect)) caretColor
  where
    prefixWidth = Platform.measureText (take textBoxCaret textBoxText)

drawMeasuredSelection :: Rect -> TextBoxState -> Render world IO ()
drawMeasuredSelection rect textState =
  case selectedRange textState of
    Nothing -> pure ()
    Just (start, end) ->
      fillRect (Rect (x rect + beforeWidth start) (y rect) (selectionWidth start end) (height rect)) selectionColor
  where
    beforeWidth start = Platform.measureText (take start (textBoxText textState))
    selectionWidth start end = Platform.measureText (take (end - start) (drop start (textBoxText textState)))

closestCaretIndex :: String -> Double -> Int
closestCaretIndex text targetX =
  fst (minimumBy (comparing snd) measured)
  where
    measured = fmap measureIndex [0 .. length text]
    measureIndex index =
      let prefixWidth = Platform.measureText (take index text)
       in (index, abs (prefixWidth - targetX))

textColor :: String
textColor = "#20242a"

selectionColor :: String
selectionColor = "#cfe3ff"

caretColor :: String
caretColor = "#20242a"
