module Halay.Geometry
  ( Constraints (..)
  , Insets (..)
  , Point (..)
  , Rect (..)
  , Size (..)
  , expandSize
  , insetConstraints
  , insetRect
  , rectContains
  , sizeRectAt
  , unconstrained
  ) where

data Point = Point
  { pointX :: Double
  , pointY :: Double
  }
  deriving (Eq, Show)

data Rect = Rect
  { x :: Double
  , y :: Double
  , width :: Double
  , height :: Double
  }
  deriving (Eq, Show)

data Size = Size
  { sizeWidth :: Double
  , sizeHeight :: Double
  }
  deriving (Eq, Show)

data Insets = Insets
  { insetTop :: Double
  , insetRight :: Double
  , insetBottom :: Double
  , insetLeft :: Double
  }
  deriving (Eq, Show)

data Constraints = Constraints
  { maxWidth :: Maybe Double
  , maxHeight :: Maybe Double
  }
  deriving (Eq, Show)

unconstrained :: Constraints
unconstrained = Constraints Nothing Nothing

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

expandSize :: Insets -> Size -> Size
expandSize Insets {insetTop, insetRight, insetBottom, insetLeft} Size {sizeWidth, sizeHeight} =
  Size
    { sizeWidth = sizeWidth + insetLeft + insetRight
    , sizeHeight = sizeHeight + insetTop + insetBottom
    }

insetConstraints :: Insets -> Constraints -> Constraints
insetConstraints Insets {insetTop, insetRight, insetBottom, insetLeft} Constraints {maxWidth, maxHeight} =
  Constraints
    { maxWidth = shrink (insetLeft + insetRight) <$> maxWidth
    , maxHeight = shrink (insetTop + insetBottom) <$> maxHeight
    }
  where
    shrink inset value =
      max 0 (value - inset)

sizeRectAt :: Point -> Size -> Rect
sizeRectAt Point {pointX, pointY} Size {sizeWidth, sizeHeight} =
  Rect pointX pointY sizeWidth sizeHeight
