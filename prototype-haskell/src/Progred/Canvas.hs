module Progred.Canvas
  ( drawCanvas
  , getViewport
  ) where

import qualified Progred.Platform as Platform
import Progred.UI
import Progred.Viewport

getViewport :: IO Viewport
getViewport =
  Viewport <$> Platform.getCanvasWidth <*> Platform.getCanvasHeight

drawCanvas :: DrawCommand -> IO ()
drawCanvas (FillRect Rect {x, y, width, height} color) =
  Platform.fillRect x y width height color
drawCanvas (StrokeRect Rect {x, y, width, height} color lineWidth) =
  Platform.strokeRect x y width height color lineWidth
drawCanvas (FillText Point {pointX, pointY} color string) =
  Platform.fillText pointX pointY color string
drawCanvas (FillTextMiddle Point {pointX, pointY} color string) =
  Platform.fillTextMiddle pointX pointY color string
