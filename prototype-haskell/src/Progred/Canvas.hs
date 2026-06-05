module Progred.Canvas
  ( Canvas
    ( clearCanvas
    , fillRect
    , fillText
    , fillTextMiddle
    , measureText
    , strokeRect
    )
  , getViewport
  ) where

import qualified Progred.Platform as Platform
import Progred.Geometry
import Progred.Viewport

class Monad m => Canvas m where
  clearCanvas :: Viewport -> m ()
  fillRect :: Rect -> String -> m ()
  strokeRect :: Rect -> String -> Double -> m ()
  fillText :: Point -> String -> String -> m ()
  fillTextMiddle :: Point -> String -> String -> m ()
  measureText :: String -> m Double

instance Canvas IO where
  clearCanvas Viewport {viewportWidth, viewportHeight} =
    Platform.clearCanvas viewportWidth viewportHeight
  fillRect Rect {x, y, width, height} =
    Platform.fillRect x y width height
  strokeRect Rect {x, y, width, height} =
    Platform.strokeRect x y width height
  fillText Point {pointX, pointY} =
    Platform.fillText pointX pointY
  fillTextMiddle Point {pointX, pointY} =
    Platform.fillTextMiddle pointX pointY
  measureText =
    Platform.measureText

getViewport :: IO Viewport
getViewport =
  Viewport <$> Platform.getCanvasWidth <*> Platform.getCanvasHeight
