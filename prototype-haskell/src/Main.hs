{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE OverloadedStrings #-}

module Main (main, hello) where

import Data.Word (Word32)
import Foreign.C.String (CString, withCString)

-- JSFFI imports. The new GHC WASM backend turns "javascript" foreign
-- imports into WASM imports the JS host can wire up.
foreign import javascript "(s) => document.getElementById('root').textContent = s"
  js_setRoot :: CString -> IO ()

foreign import javascript "(n) => console.log('clicked', n)"
  js_logClick :: Word32 -> IO ()

-- Exported so the JS host can call it on a button click.
foreign export javascript "onClick"
  hello :: Word32 -> IO ()

hello :: Word32 -> IO ()
hello n = do
  js_logClick n
  withCString ("hello from haskell — clicked " <> show n) js_setRoot

main :: IO ()
main = withCString "hello from haskell" js_setRoot
