module Progred.Widgets.TextBox
  ( TextBoxState (..)
  , textBoxState
  , textBox
  ) where

import Control.Monad (when)
import Data.List (minimumBy)
import Data.Ord (comparing)
import qualified Progred.Platform as Platform
import Progred.UI

data TextBoxState = TextBoxState
  { textBeforeCaret :: String
  , textAfterCaret :: String
  , textBoxSelectionOffset :: Int
  , textBoxDragging :: Bool
  }

textBoxState :: String -> TextBoxState
textBoxState text =
  TextBoxState
    { textBeforeCaret = text
    , textAfterCaret = ""
    , textBoxSelectionOffset = 0
    , textBoxDragging = False
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
                let newCaret = caretIndexFromX old pointerX
                let moved = setCaretIndex newCaret old
                let updated =
                      moved
                        { textBoxSelectionOffset = 0
                        , textBoxDragging = True
                        }
                pure (set updated (setFocus (Just focusId) current))
            else Nothing
        PointerMove {pointerX} ->
          if textBoxDragging (get current)
            then
              Just $ do
                let old = get current
                let anchor = caretIndex old + textBoxSelectionOffset old
                let newCaret = caretIndexFromX old pointerX
                let moved = setCaretIndex newCaret old
                let updated =
                      moved
                        { textBoxSelectionOffset = anchor - caretIndex moved
                        }
                pure (set updated current)
            else Nothing
        PointerUp {} ->
          if textBoxDragging (get current)
            then
              Just $ do
                let old = get current
                pure (set old {textBoxDragging = False} current)
            else Nothing
    onKey $ \current event ->
      if getFocus current == Just focusId
        then fmap pure (editText event current)
        else Nothing
  where
    value = get world
    selected = getFocus world == Just focusId
    textX = x rect
    textY = y rect + height rect / 2
    caretIndexFromX old pointerX =
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
            KeyCode 36 -> Just (set (moveCaretStart old) current)
            KeyCode 35 -> Just (set (moveCaretEnd old) current)
            _ -> Nothing

insertString :: String -> TextBoxState -> TextBoxState
insertString string textState =
  withoutSelection
    { textBeforeCaret = textBeforeCaret withoutSelection <> string
    , textBoxDragging = False
    }
  where
    withoutSelection = deleteSelection textState

deleteBackward :: TextBoxState -> TextBoxState
deleteBackward textState
  | hasSelection textState = deleteSelection textState
  | null (textBeforeCaret textState) =
      textState {textBoxSelectionOffset = 0, textBoxDragging = False}
  | otherwise =
      textState
        { textBeforeCaret = init (textBeforeCaret textState)
        , textBoxSelectionOffset = 0
        , textBoxDragging = False
        }

deleteForward :: TextBoxState -> TextBoxState
deleteForward textState
  | hasSelection textState = deleteSelection textState
  | otherwise =
      textState
        { textAfterCaret = drop 1 (textAfterCaret textState)
        , textBoxSelectionOffset = 0
        , textBoxDragging = False
        }

moveCaret :: Bool -> Int -> TextBoxState -> TextBoxState
moveCaret extending delta textState =
  moved
    { textBoxSelectionOffset = if extending then anchor - caretIndex moved else 0
    , textBoxDragging = False
    }
  where
    anchor = caretIndex textState + textBoxSelectionOffset textState
    moved
      | delta < 0 = moveCaretLeft textState
      | delta > 0 = moveCaretRight textState
      | otherwise = textState

moveCaretLeft :: TextBoxState -> TextBoxState
moveCaretLeft textState
  | null (textBeforeCaret textState) = textState
  | otherwise =
      textState
        { textBeforeCaret = before
        , textAfterCaret = moved : textAfterCaret textState
        }
  where
    before = init (textBeforeCaret textState)
    moved = last (textBeforeCaret textState)

moveCaretRight :: TextBoxState -> TextBoxState
moveCaretRight textState =
  case textAfterCaret textState of
    [] -> textState
    moved : after ->
      textState
        { textBeforeCaret = textBeforeCaret textState <> [moved]
        , textAfterCaret = after
        }

moveCaretStart :: TextBoxState -> TextBoxState
moveCaretStart textState =
  textState
    { textBeforeCaret = ""
    , textAfterCaret = textBoxText textState
    , textBoxSelectionOffset = 0
    , textBoxDragging = False
    }

moveCaretEnd :: TextBoxState -> TextBoxState
moveCaretEnd textState =
  textState
    { textBeforeCaret = textBoxText textState
    , textAfterCaret = ""
    , textBoxSelectionOffset = 0
    , textBoxDragging = False
    }

hasSelection :: TextBoxState -> Bool
hasSelection textState =
  case selectionText textState of
    Nothing -> False
    Just _ -> True

deleteSelection :: TextBoxState -> TextBoxState
deleteSelection textState
  | textBoxSelectionOffset textState > 0 =
      textState
        { textAfterCaret = drop (textBoxSelectionOffset textState) (textAfterCaret textState)
        , textBoxSelectionOffset = 0
        , textBoxDragging = False
        }
  | textBoxSelectionOffset textState < 0 =
      textState
        { textBeforeCaret = keepUnselectedBefore textState
        , textBoxSelectionOffset = 0
        , textBoxDragging = False
        }
  | otherwise =
      textState {textBoxDragging = False}

setCaretIndex :: Int -> TextBoxState -> TextBoxState
setCaretIndex index textState =
  textState
    { textBeforeCaret = before
    , textAfterCaret = after
    }
  where
    clampedIndex = max 0 (min (length (textBoxText textState)) index)
    (before, after) = splitAt clampedIndex (textBoxText textState)

caretIndex :: TextBoxState -> Int
caretIndex =
  length . textBeforeCaret

textBoxText :: TextBoxState -> String
textBoxText textState =
  textBeforeCaret textState <> textAfterCaret textState

drawMeasuredCaret :: Rect -> TextBoxState -> Render world IO ()
drawMeasuredCaret rect TextBoxState {textBeforeCaret} =
  fillRect (Rect (x rect + prefixWidth) (y rect) 1 (height rect)) caretColor
  where
    prefixWidth = Platform.measureText textBeforeCaret

drawMeasuredSelection :: Rect -> TextBoxState -> Render world IO ()
drawMeasuredSelection rect textState =
  case selectionText textState of
    Nothing -> pure ()
    Just (beforeSelection, selection) ->
      fillRect (Rect (x rect + Platform.measureText beforeSelection) (y rect) (Platform.measureText selection) (height rect)) selectionColor

selectionText :: TextBoxState -> Maybe (String, String)
selectionText textState
  | textBoxSelectionOffset textState > 0 =
      nonemptySelection (textBeforeCaret textState) (take (textBoxSelectionOffset textState) (textAfterCaret textState))
  | textBoxSelectionOffset textState < 0 =
      nonemptySelection beforeSelection (drop (length beforeSelection) (textBeforeCaret textState))
  | otherwise =
      Nothing
  where
    beforeSelection = keepUnselectedBefore textState

nonemptySelection :: String -> String -> Maybe (String, String)
nonemptySelection beforeSelection selection
  | null selection = Nothing
  | otherwise = Just (beforeSelection, selection)

keepUnselectedBefore :: TextBoxState -> String
keepUnselectedBefore textState =
  take keepCount (textBeforeCaret textState)
  where
    selectedCount = min (negate (textBoxSelectionOffset textState)) (length (textBeforeCaret textState))
    keepCount = length (textBeforeCaret textState) - selectedCount

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
