module Puri.Widget
  ( Widget
  ) where

import Puri.Handler
import Puri.Geometry

type Widget actionM renderM =
  Placement -> renderM (Handler actionM)
