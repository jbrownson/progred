{-# LANGUAGE LambdaCase #-}

module Progred.Widgets.Button
  ( button
  ) where

import Progred.Frame
import Progred.Geometry
import qualified Progred.KeyCode as KeyCode
import Progred.Widget

button
  :: Applicative m
  => m ()
  -> (WidgetFocus -> Frame m)
  -> Widget () m
button activate content _ rect focus actions =
  mconcat
    [ content focus
    , case focus of
        WidgetFocused -> strokeRect rect focusColor 2
        WidgetUnfocused -> mempty
    , onPointer $ \case
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then Just (widgetFocusSelf actions *> activate)
            else Nothing
        _ -> Nothing
    , onKey $ \event ->
        case focus of
          WidgetFocused -> case event of
            KeyCode code
              | code == KeyCode.enter -> Just activate
              | code == KeyCode.space -> Just activate
            _ -> Nothing
          WidgetUnfocused -> Nothing
    ]

focusColor :: String
focusColor =
  "#0a84ff"
