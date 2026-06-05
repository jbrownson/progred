{-# LANGUAGE LambdaCase #-}

module Progred.Widgets.Button
  ( button
  ) where

import Progred.Frame
import Progred.Geometry
import Progred.Canvas (Canvas)
import qualified Progred.KeyCode as KeyCode
import Progred.Widget

button
  :: (Applicative actionM, Canvas renderM)
  => actionM ()
  -> (WidgetFocus -> Frame actionM renderM)
  -> Widget () actionM renderM
button activate content _ rect focus actions =
  mconcat
    [ content focus
    , case focus of
        WidgetFocused -> strokeRect rect focusColor 2
        WidgetUnfocused -> mempty
    , onPointer $ \case
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then pure (Just (widgetFocusSelf actions *> activate))
            else pure Nothing
        _ -> pure Nothing
    , onKey $ \event ->
        case focus of
          WidgetFocused -> case event of
            KeyCode code
              | code == KeyCode.enter -> pure (Just activate)
              | code == KeyCode.space -> pure (Just activate)
            _ -> pure Nothing
          WidgetUnfocused -> pure Nothing
    ]

focusColor :: String
focusColor =
  "#0a84ff"
