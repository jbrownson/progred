{-# LANGUAGE LambdaCase #-}

module Progred.App
  ( AppM
  , Model (..)
  , initialModel
  , runAppM
  , view
  ) where

import Control.Monad.Trans.State.Strict (State, modify, runState)
import Progred.Canvas (Canvas)
import Progred.Frame
import Progred.Geometry
import qualified Progred.KeyCode as KeyCode
import Progred.Lens
import Progred.Viewport
import Progred.Widget
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

type AppM = State Model

runAppM :: AppM a -> Model -> (a, Model)
runAppM =
  runState

modifyModel :: (Model -> Model) -> AppM ()
modifyModel =
  modify

initialModel :: Model
initialModel =
  Model
    { focus = Nothing
    , count = 0
    , nameField = defaultTextBoxState {textBeforeCaret = "canvas owns focus"}
    }

view :: Canvas renderM => Viewport -> Model -> Frame AppM renderM
view viewport model =
  mconcat
    [ fillRect (Rect 0 0 (viewportWidth viewport) (viewportHeight viewport)) "#fbfbfa"
    , clearFocusOnBackground viewport
    , label (Point 32 42) "#3f454d" "Haskell/Wasm canvas UI"
    , label (Point 32 70) "#68707c" "Frame, handlers, focus, and text state are owned by Haskell."
    , label (Point 32 110) "#3f454d" ("Count: " <> show (count model))
    , framedButton model CounterButton (Rect 32 140 160 42) "Increment" (modifyModel (\world -> world {count = count world + 1}))
    , framedNameField model (Rect 32 206 300 42)
    , framedButton model ResetButton (Rect 32 272 120 42) "Reset" (modifyModel (\world -> world {count = 0}))
    , globalKeys
    ]

label :: Canvas renderM => Point -> String -> String -> Frame AppM renderM
label =
  fillText

framedButton :: Canvas renderM => Model -> FocusId -> Rect -> String -> AppM () -> Frame AppM renderM
framedButton model focusId rect text activate =
  mountWidget model unitLens focusId rect $
    button
      activate
      ( \_contentFocus -> mconcat
          [ fillRect rect background
          , strokeRect rect border 2
          , fillTextMiddle (Point (x contentRect) (y contentRect + height contentRect / 2)) "#20242a" text
          ]
      )
  where
    background = "#ffffff"
    border = "#c7cbd1"
    contentRect = insetRect (Insets 0 16 0 16) rect

framedNameField :: Canvas renderM => Model -> Rect -> Frame AppM renderM
framedNameField model rect =
  mountWidget model nameFieldLens NameField rect field
  where
    field state fieldRect fieldFocus actions =
      mconcat
        [ fillRect fieldRect "#ffffff"
        , strokeRect fieldRect (fieldBorder fieldFocus) 2
        , textBox state (insetRect (Insets 10 10 10 10) fieldRect) fieldFocus actions
        ]
    fieldBorder WidgetFocused = "#0a84ff"
    fieldBorder WidgetUnfocused = "#c7cbd1"

mountWidget
  :: Model
  -> Lens Model state
  -> FocusId
  -> Rect
  -> Widget state AppM renderM
  -> Frame AppM renderM
mountWidget model stateLens focusId rect widget =
  widget
    (lensGet stateLens model)
    rect
    (widgetFocus (lensGet focusLens model == Just focusId))
    actions
  where
    actions =
      WidgetActions
        { widgetFocusSelf = modifyModel (lensSet focusLens (Just focusId))
        , widgetSetState = applyWidgetState stateLens
        }

widgetFocus :: Bool -> WidgetFocus
widgetFocus focused =
  if focused then WidgetFocused else WidgetUnfocused

applyWidgetState :: Lens Model state -> state -> AppM ()
applyWidgetState stateLens state =
  modifyModel (lensSet stateLens state)

focusLens :: Lens Model (Maybe FocusId)
focusLens =
  Lens
    { lensGet = focus
    , lensSet = \newFocus world -> world {focus = newFocus}
    }

nameFieldLens :: Lens Model TextBoxState
nameFieldLens =
  Lens
    { lensGet = nameField
    , lensSet = \state world -> world {nameField = state}
    }

globalKeys :: Applicative renderM => Frame AppM renderM
globalKeys =
  onKey $ \case
    KeyCode code
      | code == KeyCode.tab -> pure (Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))})))
      | code == KeyCode.shiftTab -> pure (Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))})))
      | code == KeyCode.left -> pure (Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))})))
      | code == KeyCode.up -> pure (Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))})))
      | code == KeyCode.right -> pure (Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))})))
      | code == KeyCode.down -> pure (Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))})))
    _ -> pure Nothing

clearFocusOnBackground :: Applicative renderM => Viewport -> Frame AppM renderM
clearFocusOnBackground Viewport {viewportWidth, viewportHeight} =
  onPointer $ \case
    PointerDown {pointerX, pointerY} ->
      if rectContains (Rect 0 0 viewportWidth viewportHeight) pointerX pointerY
        then pure (Just (modifyModel (\world -> world {focus = Nothing})))
        else pure Nothing
    _ -> pure Nothing

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
