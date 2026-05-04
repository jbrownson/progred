{-# LANGUAGE ForeignFunctionInterface #-}

module Progred.Platform
  ( logClick
  , setRoot
  ) where

import Data.Word (Word32)
import GHC.Wasm.Prim (JSString (JSString), JSVal, toJSString)

-- JSFFI imports. The GHC WASM backend turns "javascript" foreign
-- imports into WASM imports the JS host can wire up.
foreign import javascript unsafe "document.getElementById('root').textContent = $1"
  jsSetRoot :: JSVal -> IO ()

foreign import javascript unsafe "console.log('clicked', $1)"
  logClick :: Word32 -> IO ()

setRoot :: String -> IO ()
setRoot s =
  case toJSString s of
    JSString jsString -> jsSetRoot jsString
