{-# LANGUAGE OverloadedLabels #-}
{-# LANGUAGE OverloadedStrings #-}

module Main (main) where

import Data.GI.Base (AttrOp ((:=)), new, on)
import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Data.Text (pack)
import GI.Gtk qualified as Gtk

main :: IO ()
main = do
  app <- new Gtk.Application [#applicationId := "haskell.proto.gtk", #flags := []]
  _ <- on app #activate (activate app)
  _ <- #run app (Nothing :: Maybe [String])
  pure ()

activate :: Gtk.Application -> IO ()
activate app = do
  window <- new Gtk.ApplicationWindow
    [ #application := app
    , #title := "prototype-haskell-gtk"
    , #defaultWidth := 600
    , #defaultHeight := 400
    ]

  -- Vertical layout: a label + a button.
  box <- new Gtk.Box [#orientation := Gtk.OrientationVertical, #spacing := 12, #marginTop := 24, #marginBottom := 24, #marginStart := 24, #marginEnd := 24]
  label <- new Gtk.Label [#label := "GTK4 from Haskell."]
  counter <- newIORef (0 :: Int)
  button <- new Gtk.Button [#label := "click me"]
  _ <- on button #clicked $ bump counter label
  Gtk.boxAppend box label
  Gtk.boxAppend box button
  Gtk.windowSetChild window (Just box)

  Gtk.windowPresent window

bump :: IORef Int -> Gtk.Label -> IO ()
bump counter label = do
  n <- (+ 1) <$> readIORef counter
  writeIORef counter n
  Gtk.labelSetLabel label (pack ("clicked " <> show n <> " times"))
