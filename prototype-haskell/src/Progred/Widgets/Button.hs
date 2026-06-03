module Progred.Widgets.Button
  ( button
  ) where

import Progred.UI

button
  :: Applicative m
  => FocusTarget world
  -> Rect
  -> (world -> world)
  -> Frame world m
  -> Frame world m
button focusTarget rect activate content =
  mconcat
    [ content
    , onPointer $ \current event ->
        case event of
          PointerDown {pointerX, pointerY} ->
            if rectContains rect pointerX pointerY
              then Just (pure (activate (focusTargetFocus focusTarget current)))
              else Nothing
          _ -> Nothing
    , onKey $ \current event ->
        if focusTargetIsFocused focusTarget
          then case event of
            KeyCode 13 -> Just (pure (activate current))
            KeyCode 32 -> Just (pure (activate current))
            _ -> Nothing
          else Nothing
    ]
