module Puri.Canvas
  ( Canvas
    ( clearCanvas
    , fillRect
    , fillText
    , fillTextMiddle
    , measureText
    , strokeLine
    , strokeRect
    , withClip
    , withGraphTransform
    )
  , TextMetrics (..)
  , getViewport
  ) where

import qualified Puri.Platform as Platform
import Control.Exception (bracket_)
import Puri.Geometry
import Puri.Viewport

class Monad m => Canvas m where
  clearCanvas :: Viewport -> m ()
  fillRect :: Rect -> String -> m ()
  strokeRect :: Rect -> String -> Double -> m ()
  fillText :: Point -> String -> String -> m ()
  fillTextMiddle :: Point -> String -> String -> m ()
  strokeLine :: Point -> Point -> String -> Double -> m ()
  withClip :: Rect -> m a -> m a
  withGraphTransform :: Point -> Double -> m a -> m a
  measureText :: String -> m TextMetrics

data TextMetrics = TextMetrics
  { textWidth :: Double
  , textActualBoundingBoxAscent :: Double
  , textActualBoundingBoxDescent :: Double
  , textFontBoundingBoxAscent :: Double
  , textFontBoundingBoxDescent :: Double
  }

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
  strokeLine Point {pointX = x1, pointY = y1} Point {pointX = x2, pointY = y2} =
    Platform.strokeLine x1 y1 x2 y2
  withClip Rect {x, y, width, height} =
    bracket_ (Platform.save *> Platform.clipRect x y width height) Platform.restore
  withGraphTransform Point {pointX, pointY} zoom =
    bracket_
      ( Platform.save
          *> Platform.translateCanvas pointX pointY
          *> Platform.scaleCanvas zoom zoom
      )
      Platform.restore
  measureText =
    fmap fromPlatformTextMetrics . Platform.measureText

fromPlatformTextMetrics :: Platform.TextMetrics -> TextMetrics
fromPlatformTextMetrics metrics =
  TextMetrics
    { textWidth = Platform.textWidth metrics
    , textActualBoundingBoxAscent = Platform.textActualBoundingBoxAscent metrics
    , textActualBoundingBoxDescent = Platform.textActualBoundingBoxDescent metrics
    , textFontBoundingBoxAscent = Platform.textFontBoundingBoxAscent metrics
    , textFontBoundingBoxDescent = Platform.textFontBoundingBoxDescent metrics
    }

getViewport :: IO Viewport
getViewport =
  Viewport <$> Platform.getCanvasWidth <*> Platform.getCanvasHeight
