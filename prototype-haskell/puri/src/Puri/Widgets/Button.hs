{-# LANGUAGE LambdaCase #-}

module Puri.Widgets.Button
  ( Button (..)
  , button
  ) where

import Control.Monad (when)
import Puri.Handler
import Puri.Geometry
import qualified Puri.Canvas as Canvas
import qualified Puri.KeyCode as KeyCode
import Puri.Widget

data Button actionM renderM = Button
  { buttonActivate :: actionM ()
  , buttonContent :: Bool -> Rect -> renderM ()
  , buttonFocused :: Bool
  , buttonFocus :: actionM ()
  }

button :: (Applicative actionM, Canvas.Canvas renderM) => Button actionM renderM -> Widget actionM renderM
button props =
  Widget $ \rect -> do
    buttonContent props (buttonFocused props) rect
    when (buttonFocused props) (Canvas.strokeRect rect focusColor 2)
    pure $
      mconcat
        [ onPointer $ \case
          PointerDown {pointerX, pointerY} ->
            if rectContains rect pointerX pointerY
              then Just (buttonFocus props *> buttonActivate props)
              else Nothing
          _ -> Nothing
        , onKey $ \event ->
          if buttonFocused props
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
