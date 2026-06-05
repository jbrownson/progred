module Puri.Platform
  ( clearCanvas
  , fillRect
  , fillText
  , fillTextMiddle
  , getCanvasHeight
  , getCanvasWidth
  , measureText
  , strokeRect
  ) where

clearCanvas :: Double -> Double -> IO ()
clearCanvas = undefined

fillRect :: Double -> Double -> Double -> Double -> String -> IO ()
fillRect = undefined

strokeRect :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeRect = undefined

fillText :: Double -> Double -> String -> String -> IO ()
fillText = undefined

fillTextMiddle :: Double -> Double -> String -> String -> IO ()
fillTextMiddle = undefined

getCanvasWidth :: IO Double
getCanvasWidth = undefined

getCanvasHeight :: IO Double
getCanvasHeight = undefined

measureText :: String -> IO Double
measureText = undefined
