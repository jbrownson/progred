{-# LANGUAGE LambdaCase #-}

module Progred.Widgets.TextBox
  ( TextBoxState (..)
  , defaultTextBoxState
  , textBox
  ) where

import Data.List (minimumBy)
import Data.Ord (comparing)
import Progred.Frame
import Progred.Geometry
import qualified Progred.KeyCode as KeyCode
import qualified Progred.Platform as Platform
import Progred.Widget

data TextBoxState = TextBoxState
  { textBeforeCaret :: String
  , textAfterCaret :: String
  , textBoxSelectionOffset :: Int
  , textBoxDragging :: Bool
  }

defaultTextBoxState :: TextBoxState
defaultTextBoxState =
  TextBoxState
    { textBeforeCaret = ""
    , textAfterCaret = ""
    , textBoxSelectionOffset = 0
    , textBoxDragging = False
    }

textBox
  :: Applicative m
  => Widget TextBoxState m
textBox state rect focus actions =
  mconcat
    [ selection
    , text
    , pointerDownEvents
    , focusedFrame
    , draggingFrame
    ]
  where
    textX = x rect
    textY = y rect + height rect / 2
    selection =
      drawSelection rect state
    text =
      fillTextMiddle (Point textX textY) textColor (textBoxText state)
    pointerDownEvents =
      onPointer $ \case
        PointerDown {pointerX, pointerY} -> pointerDown pointerX pointerY
        _ -> Nothing
    focusedFrame =
      case focus of
        WidgetFocused -> mconcat [caret, keyEvents]
        WidgetUnfocused -> mempty
    draggingFrame =
      if textBoxDragging state
        then draggingEvents
        else mempty
    caret =
      drawCaret rect state
    draggingEvents =
      onPointer $ \case
        PointerMove {pointerX} -> pointerMove pointerX
        PointerUp {} -> pointerUp
        _ -> Nothing
    keyEvents =
      onKey keyDown
    pointerDown pointerX pointerY =
      if rectContains rect pointerX pointerY
        then Just (widgetFocusSelf actions *> setState (startDragAt pointerX))
        else Nothing
    pointerMove pointerX =
      Just (setState (continueDragAt pointerX))
    pointerUp =
      Just (setState state {textBoxDragging = False})
    keyDown event =
      case editText event of
        Just updated -> Just (setState updated)
        Nothing -> Nothing
    caretIndexFromX pointerX =
      closestCaretIndex (textBoxText state) (pointerX - textX)
    startDragAt pointerX =
      (setCaretIndex (caretIndexFromX pointerX) state)
        { textBoxSelectionOffset = 0
        , textBoxDragging = True
        }
    continueDragAt pointerX =
      moved
        { textBoxSelectionOffset = anchor - caretIndex moved
        }
      where
        anchor = caretIndex state + textBoxSelectionOffset state
        moved = setCaretIndex (caretIndexFromX pointerX) state
    setState updated =
      widgetSetState actions updated
    editText event =
      case event of
        TextInput string -> Just (insertString string state)
        KeyCode code
          | code == KeyCode.space -> Just (insertString " " state)
          | code == KeyCode.backspace -> Just (deleteBackward state)
          | code == KeyCode.delete -> Just (deleteForward state)
          | code == KeyCode.left -> Just (moveCaret False (-1) state)
          | code == KeyCode.right -> Just (moveCaret False 1 state)
          | code == KeyCode.shiftLeft -> Just (moveCaret True (-1) state)
          | code == KeyCode.shiftRight -> Just (moveCaret True 1 state)
          | code == KeyCode.home -> Just (moveCaretStart state)
          | code == KeyCode.end -> Just (moveCaretEnd state)
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

drawCaret :: Rect -> TextBoxState -> Frame m
drawCaret rect TextBoxState {textBeforeCaret} =
  fillRect (Rect (x rect + prefixWidth) (y rect) 1 (height rect)) caretColor
  where
    prefixWidth = Platform.measureText textBeforeCaret

drawSelection :: Rect -> TextBoxState -> Frame m
drawSelection rect textState =
  case selectionText textState of
    Nothing -> mempty
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
