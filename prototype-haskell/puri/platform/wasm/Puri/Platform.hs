{-# LANGUAGE ForeignFunctionInterface #-}

module Puri.Platform
  ( clearCanvas
  , fillRect
  , fillText
  , fillTextMiddle
  , getCanvasHeight
  , getCanvasWidth
  , measureText
  , strokeRect
  , TextMetrics (..)
  ) where

import GHC.Wasm.Prim (JSString (JSString), JSVal, toJSString)

data TextMetrics = TextMetrics
  { textWidth :: Double
  , textActualBoundingBoxAscent :: Double
  , textActualBoundingBoxDescent :: Double
  , textFontBoundingBoxAscent :: Double
  , textFontBoundingBoxDescent :: Double
  }

-- JSFFI imports. The GHC WASM backend turns "javascript" foreign
-- imports into WASM imports the JS host can wire up.
foreign import javascript unsafe "window.puriCanvas.clear($1, $2)"
  clearCanvas :: Double -> Double -> IO ()

foreign import javascript unsafe "window.puriCanvas.fillRect($1, $2, $3, $4, $5)"
  jsFillRect :: Double -> Double -> Double -> Double -> JSVal -> IO ()

foreign import javascript unsafe "window.puriCanvas.strokeRect($1, $2, $3, $4, $5, $6)"
  jsStrokeRect :: Double -> Double -> Double -> Double -> JSVal -> Double -> IO ()

foreign import javascript unsafe "window.puriCanvas.fillText($1, $2, $3, $4)"
  jsFillText :: Double -> Double -> JSVal -> JSVal -> IO ()

foreign import javascript unsafe "window.puriCanvas.fillTextMiddle($1, $2, $3, $4)"
  jsFillTextMiddle :: Double -> Double -> JSVal -> JSVal -> IO ()

foreign import javascript unsafe "window.puriCanvas.measureText($1)"
  jsMeasureText :: JSVal -> IO Double

foreign import javascript unsafe "window.puriCanvas.measureTextActualAscent($1)"
  jsMeasureTextActualAscent :: JSVal -> IO Double

foreign import javascript unsafe "window.puriCanvas.measureTextActualDescent($1)"
  jsMeasureTextActualDescent :: JSVal -> IO Double

foreign import javascript unsafe "window.puriCanvas.measureTextFontAscent($1)"
  jsMeasureTextFontAscent :: JSVal -> IO Double

foreign import javascript unsafe "window.puriCanvas.measureTextFontDescent($1)"
  jsMeasureTextFontDescent :: JSVal -> IO Double

foreign import javascript unsafe "window.puriCanvas.width()"
  getCanvasWidth :: IO Double

foreign import javascript unsafe "window.puriCanvas.height()"
  getCanvasHeight :: IO Double

fillRect :: Double -> Double -> Double -> Double -> String -> IO ()
fillRect x y width height color =
  case toJSString color of
    JSString jsString -> jsFillRect x y width height jsString

strokeRect :: Double -> Double -> Double -> Double -> String -> Double -> IO ()
strokeRect x y width height color lineWidth =
  case toJSString color of
    JSString jsString -> jsStrokeRect x y width height jsString lineWidth

fillText :: Double -> Double -> String -> String -> IO ()
fillText x y color string =
  case (toJSString color, toJSString string) of
    (JSString colorString, JSString textString) -> jsFillText x y colorString textString

fillTextMiddle :: Double -> Double -> String -> String -> IO ()
fillTextMiddle x y color string =
  case (toJSString color, toJSString string) of
    (JSString colorString, JSString textString) -> jsFillTextMiddle x y colorString textString

measureText :: String -> IO TextMetrics
measureText string =
  case toJSString string of
    JSString textString -> do
      width <- jsMeasureText textString
      actualAscent <- jsMeasureTextActualAscent textString
      actualDescent <- jsMeasureTextActualDescent textString
      fontAscent <- jsMeasureTextFontAscent textString
      fontDescent <- jsMeasureTextFontDescent textString
      pure
        TextMetrics
          { textWidth = width
          , textActualBoundingBoxAscent = actualAscent
          , textActualBoundingBoxDescent = actualDescent
          , textFontBoundingBoxAscent = fontAscent
          , textFontBoundingBoxDescent = fontDescent
          }
