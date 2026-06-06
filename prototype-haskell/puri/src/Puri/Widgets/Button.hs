{-# LANGUAGE LambdaCase #-}

module Puri.Widgets.Button
  ( button
  ) where

import Control.Monad (when)
import Puri.Handler
import Puri.Geometry
import qualified Puri.Canvas as Canvas
import qualified Puri.KeyCode as KeyCode
import Puri.Widget

button
  :: (Applicative actionM, Canvas.Canvas renderM)
  => actionM ()
  -> (WidgetFocus -> renderM ())
  -> Widget () actionM renderM
button activate content _ rect focus actions = do
  content focus
  when (widgetIsFocused focus) (Canvas.strokeRect rect focusColor 2)
  pure $
    mconcat
      [ onPointer $ \case
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then Just (widgetFocusSelf actions *> activate)
            else Nothing
        _ -> Nothing
      , onKey $ \event ->
        if widgetIsFocused focus
          then keyActivate event
          else Nothing
      ]
  where
    keyActivate event =
      case event of
        KeyCode _modifiers code
          | code == KeyCode.enter -> Just activate
          | code == KeyCode.space -> Just activate
        _ -> Nothing

focusColor :: String
focusColor =
  "#0a84ff"
