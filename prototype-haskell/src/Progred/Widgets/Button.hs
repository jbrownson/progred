{-# LANGUAGE LambdaCase #-}

module Progred.Widgets.Button
  ( button
  ) where

import Control.Monad (when)
import Progred.Handler
import Progred.Geometry
import qualified Progred.Canvas as Canvas
import qualified Progred.KeyCode as KeyCode
import Progred.Widget

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
        KeyCode code
          | code == KeyCode.enter -> Just activate
          | code == KeyCode.space -> Just activate
        _ -> Nothing

focusColor :: String
focusColor =
  "#0a84ff"
