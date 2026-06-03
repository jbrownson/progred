module Progred.App
  ( Model (..)
  , initialModel
  , view
  ) where

import Progred.UI
import Progred.Viewport
import Progred.Widgets.Button
import Progred.Widgets.TextBox

data FocusId
  = CounterButton
  | NameField
  | ResetButton
  deriving (Bounded, Enum, Eq)

data Model = Model
  { focus :: Maybe FocusId
  , count :: Int
  , nameField :: TextBoxState
  }

initialModel :: Model
initialModel =
  Model
    { focus = Nothing
    , count = 0
    , nameField = defaultTextBoxState {textBeforeCaret = "canvas owns focus"}
    }

view :: Viewport -> Model -> Frame Model IO
view viewport model =
  mconcat
    [ fillRect (Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)) "#fbfbfa"
    , clearFocusOnBackground viewport
    , label (Point 32 42) "#3f454d" "Haskell/Wasm canvas UI"
    , label (Point 32 70) "#68707c" "Frame, handlers, focus, and text state are owned by Haskell."
    , label (Point 32 110) "#3f454d" ("Count: " <> show (count model))
    , framedButton model CounterButton (Rect 32 140 160 42) "Increment" (\world -> world {count = count world + 1})
    , framedNameField model (Rect 32 206 300 42)
    , framedButton model ResetButton (Rect 32 272 120 42) "Reset" (\world -> world {count = 0})
    , globalKeys
    ]

focusTarget :: Model -> FocusId -> FocusTarget Model
focusTarget model focusId =
  FocusTarget
    { focusTargetIsFocused = focus model == Just focusId
    , focusTargetFocus = \current -> current {focus = Just focusId}
    }

label :: Point -> String -> String -> Frame Model IO
label =
  fillText

framedButton :: Model -> FocusId -> Rect -> String -> (Model -> Model) -> Frame Model IO
framedButton model focusId rect text activate =
  button target rect activate $
    mconcat
      [ fillRect rect background
      , strokeRect rect border 2
      , fillTextMiddle (Point (x contentRect) (y contentRect + height contentRect / 2)) "#20242a" text
      ]
  where
    target = focusTarget model focusId
    selected = focusTargetIsFocused target
    background = if selected then "#dbeaff" else "#ffffff"
    border = if selected then "#0a84ff" else "#c7cbd1"
    contentRect = insetRect (Insets 0 16 0 16) rect

framedNameField :: Model -> Rect -> Frame Model IO
framedNameField model rect =
  mconcat
    [ fillRect rect "#ffffff"
    , strokeRect rect border 2
    , textBox
        (focusTarget model NameField)
        (insetRect (Insets 10 10 10 10) rect)
        (nameField model)
        (\state world -> world {nameField = state})
    ]
  where
    border =
      case focus model of
        Just NameField -> "#0a84ff"
        _ -> "#c7cbd1"

globalKeys :: Frame Model IO
globalKeys =
  onKey $ \world event ->
    case event of
      KeyCode 9 -> Just (pure world {focus = Just (nextFocus (focus world))})
      KeyCode 1009 -> Just (pure world {focus = Just (previousFocus (focus world))})
      KeyCode 37 -> Just (pure world {focus = Just (previousFocus (focus world))})
      KeyCode 38 -> Just (pure world {focus = Just (previousFocus (focus world))})
      KeyCode 39 -> Just (pure world {focus = Just (nextFocus (focus world))})
      KeyCode 40 -> Just (pure world {focus = Just (nextFocus (focus world))})
      _ -> Nothing

clearFocusOnBackground :: Viewport -> Frame Model IO
clearFocusOnBackground Viewport {viewportWidth, viewportHeight} =
  onPointer $ \world event ->
    case event of
      PointerDown {pointerX, pointerY} ->
        if rectContains (Rect 0 0 viewportWidth viewportHeight) pointerX pointerY
          then Just (pure world {focus = Nothing})
          else Nothing
      _ -> Nothing

nextFocus :: Maybe FocusId -> FocusId
nextFocus Nothing =
  minBound
nextFocus (Just focusId)
  | focusId == maxBound = minBound
  | otherwise = succ focusId

previousFocus :: Maybe FocusId -> FocusId
previousFocus Nothing =
  maxBound
previousFocus (Just focusId)
  | focusId == minBound = maxBound
  | otherwise = pred focusId
