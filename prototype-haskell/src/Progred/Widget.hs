{-# LANGUAGE FunctionalDependencies #-}

module Progred.Widget
  ( Widget
  , WidgetChangeEvent (..)
  , WidgetFocus (..)
  , WidgetActions (..)
  ) where

import Progred.Frame
import Progred.Geometry

type Widget state widgetM =
  state
    -> Rect
    -> WidgetFocus
    -> (WidgetChangeEvent state -> widgetM ())
    -> Frame widgetM

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused

data WidgetChangeEvent state = WidgetChangeEvent
  { widgetChangeOld :: state
  , widgetChangeNew :: state
  }

class WidgetActions state appM widgetM | widgetM -> state appM where
  putState :: state -> widgetM ()
  focusSelf :: widgetM ()
  liftApp :: appM a -> widgetM a
