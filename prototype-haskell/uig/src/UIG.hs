module UIG
  ( Insets (..)
  , Placement (..)
  , Point (..)
  , Rect (..)
  , Size (..)
  , expandSize
  , insetRect
  , intersectRect
  , clip
  , rectContains
  , rootPlacement
  , sizeRectAt
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

data Placement = Placement
  { placementRect :: Rect
  , clipRect :: Rect
  }
  deriving (Eq, Show)

rootPlacement :: Rect -> Placement
rootPlacement rect =
  Placement rect rect

clip :: Rect -> Placement -> Placement
clip bounds placement =
  placement {clipRect = intersectRect bounds (clipRect placement)}

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

sizeRectAt :: Point -> Size -> Rect
sizeRectAt Point {pointX, pointY} Size {sizeWidth, sizeHeight} =
  Rect pointX pointY sizeWidth sizeHeight

intersectRect :: Rect -> Rect -> Rect
intersectRect Rect {x = leftX, y = topY, width = leftWidth, height = topHeight} Rect {x = rightX, y = rightY, width = rightWidth, height = rightHeight} =
  let x = max leftX rightX
      y = max topY rightY
      right = min (leftX + leftWidth) (rightX + rightWidth)
      bottom = min (topY + topHeight) (rightY + rightHeight)
   in Rect x y (max 0 (right - x)) (max 0 (bottom - y))