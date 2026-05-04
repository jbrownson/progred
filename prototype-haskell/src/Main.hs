{-# LANGUAGE ForeignFunctionInterface #-}

module Main (main, start, hello) where

import Data.Word (Word32)
import GHC.Wasm.Prim (JSString (JSString), JSVal, toJSString)

-- JSFFI imports. The new GHC WASM backend turns "javascript" foreign
-- imports into WASM imports the JS host can wire up.
foreign import javascript unsafe "document.getElementById('root').textContent = $1"
  js_setRoot :: JSVal -> IO ()

foreign import javascript unsafe "console.log('clicked', $1)"
  js_logClick :: Word32 -> IO ()

foreign export javascript "start"
  start :: IO ()

-- Exported so the JS host can call it on a button click.
foreign export javascript "onClick"
  hello :: Word32 -> IO ()

start :: IO ()
start = setRoot "hello from haskell"

hello :: Word32 -> IO ()
hello n = do
  js_logClick n
  setRoot ("hello from haskell - clicked " <> show n)

setRoot :: String -> IO ()
setRoot s =
  case toJSString s of
    JSString jsString -> js_setRoot jsString

main :: IO ()
main = start
