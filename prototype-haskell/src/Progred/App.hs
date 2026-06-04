module Progred.App
  ( AppM
  , Model (..)
  , initialModel
  , runAppM
  , view
  ) where

import Progred.Frame
import Progred.Geometry
import Progred.Viewport
import Progred.Widget
import Progred.Widget.Interpreter
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

newtype AppM a = AppM
  { runAppM :: Model -> IO (a, Model)
  }

instance Functor AppM where
  fmap f action =
    AppM $ \model -> do
      (value, updated) <- runAppM action model
      pure (f value, updated)

instance Applicative AppM where
  pure value =
    AppM $ \model -> pure (value, model)
  function <*> argument =
    AppM $ \model -> do
      (f, afterFunction) <- runAppM function model
      (value, afterArgument) <- runAppM argument afterFunction
      pure (f value, afterArgument)

instance Monad AppM where
  action >>= f =
    AppM $ \model -> do
      (value, updated) <- runAppM action model
      runAppM (f value) updated

modifyModel :: (Model -> Model) -> AppM ()
modifyModel f =
  AppM $ \model -> pure ((), f model)

initialModel :: Model
initialModel =
  Model
    { focus = Nothing
    , count = 0
    , nameField = defaultTextBoxState {textBeforeCaret = "canvas owns focus"}
    }

view :: Viewport -> Model -> Frame AppM
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

label :: Point -> String -> String -> Frame AppM
label =
  fillText

framedButton :: Model -> FocusId -> Rect -> String -> AppM () -> Frame AppM
framedButton model focusId rect text activate =
  runWidgetFrame (statelessWidgetEnv focusId) $
    button
      ButtonParams
        { buttonActivate = activate
        , buttonContent =
            mconcat
              [ fillRect rect background
              , strokeRect rect border 2
              , fillTextMiddle (Point (x contentRect) (y contentRect + height contentRect / 2)) "#20242a" text
              ]
      }
      ()
      rect
      (widgetFocus focused)
      applyWidgetChange
  where
    focused = focus model == Just focusId
    background = if focused then "#dbeaff" else "#ffffff"
    border = if focused then "#0a84ff" else "#c7cbd1"
    contentRect = insetRect (Insets 0 16 0 16) rect

framedNameField :: Model -> Rect -> Frame AppM
framedNameField model rect =
  runWidgetFrame (textBoxWidgetEnv NameField (\state world -> world {nameField = state})) $
    mconcat
      [ fillRect rect "#ffffff"
      , strokeRect rect border 2
      , textBox (nameField model) (insetRect (Insets 10 10 10 10) rect) (widgetFocus focused) applyWidgetChange
      ]
  where
    focused =
      case focus model of
        Just NameField -> True
        _ -> False
    border = if focused then "#0a84ff" else "#c7cbd1"

widgetFocus :: Bool -> WidgetFocus
widgetFocus focused =
  if focused then WidgetFocused else WidgetUnfocused

applyWidgetChange :: WidgetActions state appM widgetM => WidgetChangeEvent state -> widgetM ()
applyWidgetChange event =
  putState (widgetChangeNew event)

statelessWidgetEnv :: FocusId -> WidgetEnv () AppM
statelessWidgetEnv focusId =
  WidgetEnv
    { widgetEnvPutState = \() -> pure ()
    , widgetEnvFocusSelf = focusSelfInModel focusId
    }

textBoxWidgetEnv :: FocusId -> (TextBoxState -> Model -> Model) -> WidgetEnv TextBoxState AppM
textBoxWidgetEnv focusId set =
  WidgetEnv
    { widgetEnvPutState = \state -> modifyModel (set state)
    , widgetEnvFocusSelf = focusSelfInModel focusId
    }

focusSelfInModel :: FocusId -> AppM ()
focusSelfInModel focusId =
  modifyModel (\world -> world {focus = Just focusId})

globalKeys :: Frame AppM
globalKeys =
  onKey $ \event ->
    case event of
      KeyCode 9 -> Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))}))
      KeyCode 1009 -> Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))}))
      KeyCode 37 -> Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))}))
      KeyCode 38 -> Just (modifyModel (\world -> world {focus = Just (previousFocus (focus world))}))
      KeyCode 39 -> Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))}))
      KeyCode 40 -> Just (modifyModel (\world -> world {focus = Just (nextFocus (focus world))}))
      _ -> Nothing

clearFocusOnBackground :: Viewport -> Frame AppM
clearFocusOnBackground Viewport {viewportWidth, viewportHeight} =
  onPointer $ \event ->
    case event of
      PointerDown {pointerX, pointerY} ->
        if rectContains (Rect 0 0 viewportWidth viewportHeight) pointerX pointerY
          then Just (modifyModel (\world -> world {focus = Nothing}))
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
