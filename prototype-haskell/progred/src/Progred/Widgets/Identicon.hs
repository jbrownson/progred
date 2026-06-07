module Progred.Widgets.Identicon
  ( IdenticonPalette (..)
  , identicon
  , identiconWithPalette
  ) where

import Data.Bits ((.&.), (.|.), shiftL, shiftR)
import Data.UUID.Types (UUID)
import qualified Data.UUID.Types as UUID
import Puri.Geometry
import qualified Puri.Canvas as Canvas

data IdenticonPalette
  = BalancedPalette
  | OceanPalette
  | EmberPalette
  | VioletPalette
  | SlatePalette
  | CandyPalette
  | MossPalette
  | SolarPalette

gridSize :: Int
gridSize =
  6

identicon :: Canvas.Canvas renderM => UUID -> Rect -> renderM ()
identicon uuid =
  identiconWithPalette (paletteFromUUID uuid) uuid

identiconWithPalette :: Canvas.Canvas renderM => IdenticonPalette -> UUID -> Rect -> renderM ()
identiconWithPalette palette uuid rect =
  do
    mapM_ drawCell (zip [0 ..] cellIndexes)
  where
    inner = insetRect (Insets 1 1 1 1) rect
    cellWidth = width inner / fromIntegral gridSize
    cellHeight = height inner / fromIntegral gridSize
    bits = uuidInteger uuid

    cellIndexes =
      [ (row, col)
      | row <- [0 .. gridSize - 1]
      , col <- [0 .. gridSize - 1]
      , isVisibleCell row col
      ]

    drawCell (cellIndex, (row, col)) =
      drawIdenticonCell
        (cellRect row col)
        (cellColor palette (cellState bits cellIndex))

    cellRect row col =
      Rect
        { x = x inner + fromIntegral col * cellWidth
        , y = y inner + fromIntegral row * cellHeight
        , width = cellWidth
        , height = cellHeight
        }

paletteFromUUID :: UUID -> IdenticonPalette
paletteFromUUID uuid =
  case (fromInteger (uuidInteger uuid .&. 0x7) :: Int) of
    0 -> BalancedPalette
    1 -> OceanPalette
    2 -> EmberPalette
    3 -> VioletPalette
    4 -> SlatePalette
    5 -> CandyPalette
    6 -> MossPalette
    _ -> SolarPalette

drawIdenticonCell :: Canvas.Canvas renderM => Rect -> String -> renderM ()
drawIdenticonCell rect =
  Canvas.fillRect
    Rect
      { x = x rect
      , y = y rect
      , width = width rect * 2 / 3
      , height = height rect * 2 / 3
      }

isVisibleCell :: Int -> Int -> Bool
isVisibleCell row col =
  row >= 0
    && row < gridSize
    && col >= 0
    && col < gridSize
    && not ((row == 0 || row == gridSize - 1) && (col == 0 || col == gridSize - 1))

cellState :: Integer -> Int -> Int
cellState bits cellIndex =
  fromInteger ((bits `shiftR` shift) .&. 0xf)
  where
    shift = 124 - cellIndex * 4

cellColor :: IdenticonPalette -> Int -> String
cellColor palette state =
  case palette of
    BalancedPalette -> balancedColor state
    OceanPalette -> oceanColor state
    EmberPalette -> emberColor state
    VioletPalette -> violetColor state
    SlatePalette -> slateColor state
    CandyPalette -> candyColor state
    MossPalette -> mossColor state
    SolarPalette -> solarColor state

balancedColor :: Int -> String
balancedColor state =
  colorAt state
    [ "#f5f7fa", "#d8e2f0", "#93b7e3", "#477dbb"
    , "#d7ecdf", "#79c99e", "#27946e", "#155844"
    , "#f5dfc4", "#eba65c", "#d86932", "#86391f"
    , "#ead8f2", "#b486cf", "#7746a4", "#2f3440"
    ]

oceanColor :: Int -> String
oceanColor state =
  colorAt state
    [ "#f3fbff", "#d5f0fb", "#9ad8ee", "#4fa9cc"
    , "#d8f4ef", "#86d6c8", "#2aa798", "#0b615b"
    , "#e7e3ff", "#aaa0ee", "#6d5fc7", "#382f78"
    , "#eef5f7", "#aabec7", "#5c7682", "#263b45"
    ]

emberColor :: Int -> String
emberColor state =
  colorAt state
    [ "#fff6ec", "#f7dcb9", "#eead6d", "#cb6d2f"
    , "#ffe5dc", "#f6a58e", "#df5d45", "#8b2e26"
    , "#f5efd9", "#d9bd72", "#a7862f", "#5d4919"
    , "#f1e4ef", "#c88abf", "#8d4e84", "#3c2a37"
    ]

violetColor :: Int -> String
violetColor state =
  colorAt state
    [ "#fbf6ff", "#e5d7f4", "#bfa1df", "#865bb2"
    , "#f3e6ff", "#cb95ef", "#9957d1", "#562b81"
    , "#e8ecff", "#a8b8f0", "#6578cf", "#303c7a"
    , "#f3eef5", "#b9a3bf", "#745d7d", "#2d2633"
    ]

slateColor :: Int -> String
slateColor state =
  colorAt state
    [ "#f8fafc", "#e5e9ee", "#c4ccd6", "#8d98a8"
    , "#eef4f2", "#c6d9d4", "#8ca9a0", "#536b63"
    , "#f5f2ed", "#d8d0c3", "#a69a88", "#6b6257"
    , "#eff1f5", "#b8bec8", "#777f8d", "#2f3440"
    ]

candyColor :: Int -> String
candyColor state =
  colorAt state
    [ "#fff7fb", "#ffc7df", "#f86aa6", "#bd1f6f"
    , "#f0f7ff", "#9bd7ff", "#33a4f4", "#145a9c"
    , "#f4fff3", "#a8ef9e", "#49bf5b", "#1d7130"
    , "#fffbe9", "#f8d855", "#f19922", "#493044"
    ]

mossColor :: Int -> String
mossColor state =
  colorAt state
    [ "#fbfbef", "#dfe8b6", "#b4c86e", "#6f8731"
    , "#edf8ea", "#aad69b", "#5da45b", "#2f6138"
    , "#eef6ed", "#bad4bd", "#789c82", "#425b49"
    , "#f2ece2", "#c8ae83", "#897047", "#312d24"
    ]

solarColor :: Int -> String
solarColor state =
  colorAt state
    [ "#fffaf0", "#ffe8a8", "#ffc247", "#cc7a00"
    , "#fff0da", "#ffbd72", "#f36d28", "#9c3219"
    , "#f0f7ff", "#9fcfff", "#438dea", "#204e9a"
    , "#f8f4e6", "#c9b878", "#82703a", "#2f2b20"
    ]

colorAt :: Int -> [String] -> String
colorAt index colors =
  colors !! index

uuidInteger :: UUID -> Integer
uuidInteger uuid =
  case UUID.toWords uuid of
    (word0, word1, word2, word3) ->
      shiftL (toInteger word0) 96
        .|. shiftL (toInteger word1) 64
        .|. shiftL (toInteger word2) 32
        .|. toInteger word3
