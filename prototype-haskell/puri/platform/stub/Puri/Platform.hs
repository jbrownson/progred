module Puri.Platform
  ( clearCanvas
  , clipRect
  , fillRect
  , fillText
  , fillTextMiddle
  , getCanvasHeight
  , getCanvasWidth
  , measureText
  , restore
  , save
  , strokeLine
  , strokeRect
  , TextMetrics (..)
  ) where

data TextMetrics = TextMetrics
  { textWidth :: Double
  , textActualBoundingBoxAscent :: Double
  , textActualBoundingBoxDescent :: Double
  , textFontBoundingBoxAscent :: Double
  , textFontBoundingBoxDescent :: Double
  }

clearCanvas :: Double -> Double -> IO ()
clearCanvas = undefined

save :: IO ()
save = undefined

restore :: IO ()
restore = undefined

clipRect :: Double -> Double -> Double -> Double -> IO ()
clipRect = undefined

fillRect :: Double -> Double -> Double -> Double -> String -> IO ()
fillRect = undefined

strokeRect :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeRect = undefined

strokeLine :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeLine = undefined

fillText :: Double -> Double -> String -> String -> IO ()
fillText = undefined

fillTextMiddle :: Double -> Double -> String -> String -> IO ()
fillTextMiddle = undefined

getCanvasWidth :: IO Double
getCanvasWidth = undefined

getCanvasHeight :: IO Double
getCanvasHeight = undefined

measureText :: String -> IO TextMetrics
measureText = undefined
