{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Platform
  ( clearCanvas
  , fillRect
  , fillText
  , strokeRect
  ) where

import GHC.Wasm.Prim (JSString (JSString), JSVal, toJSString)

-- JSFFI imports. The GHC WASM backend turns "javascript" foreign
-- imports into WASM imports the JS host can wire up.
foreign import javascript unsafe "window.progredCanvas.clear($1, $2)"
  clearCanvas :: Double -> Double -> IO ()

foreign import javascript unsafe "window.progredCanvas.fillRect($1, $2, $3, $4, $5)"
  jsFillRect :: Double -> Double -> Double -> Double -> JSVal -> IO ()

foreign import javascript unsafe "window.progredCanvas.strokeRect($1, $2, $3, $4, $5, $6)"
  jsStrokeRect :: Double -> Double -> Double -> Double -> JSVal -> Double -> IO ()

foreign import javascript unsafe "window.progredCanvas.fillText($1, $2, $3, $4)"
  jsFillText :: Double -> Double -> JSVal -> JSVal -> IO ()

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
