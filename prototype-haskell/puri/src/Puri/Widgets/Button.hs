{-# LANGUAGE LambdaCase #-}

module Puri.Widgets.Button
  ( ButtonProps (..)
  , button
  ) where

import Control.Monad (when)
import Puri.Handler
import Puri.Geometry
import qualified Puri.Canvas as Canvas
import qualified Puri.KeyCode as KeyCode
import Puri.Widget

data ButtonProps actionM renderM = ButtonProps
  { buttonActivate :: actionM ()
  , buttonContent :: WidgetFocus -> renderM ()
  , buttonFocus :: WidgetFocus
  , buttonFocusSelf :: actionM ()
  }

button :: (Applicative actionM, Canvas.Canvas renderM) => Widget (ButtonProps actionM renderM) actionM renderM
button =
  Widget $ \props rect -> do
    let focus = buttonFocus props
    buttonContent props focus
    when (widgetIsFocused focus) (Canvas.strokeRect rect focusColor 2)
    pure $
      mconcat
        [ onPointer $ \case
          PointerDown {pointerX, pointerY} ->
            if rectContains rect pointerX pointerY
              then Just (buttonFocusSelf props *> buttonActivate props)
              else Nothing
          _ -> Nothing
        , onKey $ \event ->
          if widgetIsFocused focus
            then keyActivate (buttonActivate props) event
            else Nothing
        ]

keyActivate :: actionM () -> KeyEvent -> Maybe (actionM ())
keyActivate activate event =
  case event of
    KeyCode _modifiers code
      | code == KeyCode.enter -> Just activate
      | code == KeyCode.space -> Just activate
    _ -> Nothing

focusColor :: String
focusColor =
  "#0a84ff"
