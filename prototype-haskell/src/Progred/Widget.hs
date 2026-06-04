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
    -> (WidgetChangeEvent state widgetM -> widgetM ())
    -> Frame widgetM

data WidgetFocus
  = WidgetFocused
  | WidgetUnfocused

data WidgetChangeEvent state widgetM = WidgetChangeEvent
  { widgetChangeOldState :: state
  , widgetChangeNewState :: state
  , applyWidgetChange :: widgetM ()
  }

class WidgetActions state actionM widgetM | widgetM -> state actionM where
  putState :: state -> widgetM ()
  focusSelf :: widgetM ()
  liftAction :: actionM a -> widgetM a
