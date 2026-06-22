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
  , scaleCanvas
  , strokeLine
  , translateCanvas
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
clearCanvas _ _ =
  pure ()

save :: IO ()
save =
  pure ()

restore :: IO ()
restore =
  pure ()

translateCanvas :: Double -> Double -> IO ()
translateCanvas _ _ =
  pure ()

scaleCanvas :: Double -> Double -> IO ()
scaleCanvas _ _ =
  pure ()

clipRect :: Double -> Double -> Double -> Double -> IO ()
clipRect _ _ _ _ =
  pure ()

fillRect :: Double -> Double -> Double -> Double -> String -> IO ()
fillRect _ _ _ _ _ =
  pure ()

strokeRect :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeRect _ _ _ _ _ _ =
  pure ()

strokeLine :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeLine _ _ _ _ _ _ =
  pure ()

fillText :: Double -> Double -> String -> String -> IO ()
fillText _ _ _ _ =
  pure ()

fillTextMiddle :: Double -> Double -> String -> String -> IO ()
fillTextMiddle _ _ _ _ =
  pure ()

getCanvasWidth :: IO Double
getCanvasWidth =
  pure 800

getCanvasHeight :: IO Double
getCanvasHeight =
  pure 600

measureText :: String -> IO TextMetrics
measureText string =
  pure
    TextMetrics
      { textWidth = fromIntegral (length string) * 8
      , textActualBoundingBoxAscent = 10
      , textActualBoundingBoxDescent = 3
      , textFontBoundingBoxAscent = 11
      , textFontBoundingBoxDescent = 3
      }