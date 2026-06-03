module Progred.Widgets.Button
  ( button
  ) where

import Progred.UI

button
  :: Eq focus
  => (world -> Maybe focus)
  -> (Maybe focus -> world -> world)
  -> world
  -> focus
  -> Rect
  -> (world -> world)
  -> Render world IO ()
  -> Render world IO ()
button getFocus setFocus _world focusId rect activate content =
  do
    content
    onPointer $ \current event ->
      case event of
        PointerDown {pointerX, pointerY} ->
          if rectContains rect pointerX pointerY
            then Just (pure (activate (setFocus (Just focusId) current)))
            else Nothing
        _ -> Nothing
    onKey $ \current event ->
      if getFocus current == Just focusId
        then case event of
          KeyCode 13 -> Just (pure (activate current))
          KeyCode 32 -> Just (pure (activate current))
          _ -> Nothing
        else Nothing
