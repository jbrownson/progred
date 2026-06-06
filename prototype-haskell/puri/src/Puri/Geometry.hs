module Puri.Geometry
  ( Insets (..)
  , Point (..)
  , Rect (..)
  , insetRect
  , rectContains
  ) where

data Point = Point
  { pointX :: Double
  , pointY :: Double
  }

data Rect = Rect
  { x :: Double
  , y :: Double
  , width :: Double
  , height :: Double
  }

data Insets = Insets
  { insetTop :: Double
  , insetRight :: Double
  , insetBottom :: Double
  , insetLeft :: Double
  }

rectContains :: Rect -> Double -> Double -> Bool
rectContains Rect {x, y, width, height} px py =
  px >= x
    && px <= x + width
    && py >= y
    && py <= y + height

insetRect :: Insets -> Rect -> Rect
insetRect Insets {insetTop, insetRight, insetBottom, insetLeft} Rect {x, y, width, height} =
  Rect
    { x = x + insetLeft
    , y = y + insetTop
    , width = width - insetLeft - insetRight
    , height = height - insetTop - insetBottom
    }
