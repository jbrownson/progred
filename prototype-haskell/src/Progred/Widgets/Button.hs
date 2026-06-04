{-# LANGUAGE LambdaCase #-}

module Progred.Widgets.Button
  ( button
  ) where

import Progred.Frame
import Progred.Geometry
import qualified Progred.KeyCode as KeyCode
import Progred.Widget

button
  :: (Applicative widgetM, WidgetActions () appM widgetM)
  => appM ()
  -> Frame widgetM
  -> Widget () widgetM
button activate content _ rect focus _onChange =
  mconcat
    [ content
    , onPointer $ \case
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then Just (focusSelf *> liftApp activate)
            else Nothing
        _ -> Nothing
    , onKey $ \event ->
        case focus of
          WidgetFocused -> case event of
            KeyCode code
              | code == KeyCode.enter -> Just (liftApp activate)
              | code == KeyCode.space -> Just (liftApp activate)
            _ -> Nothing
          WidgetUnfocused -> Nothing
    ]
