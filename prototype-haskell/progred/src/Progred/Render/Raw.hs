module Progred.Render.Raw
  ( rawDocument
  , renderRawDocument
  , renderRawGraph
  ) where

import Data.Map.Strict (toList)
import Data.Set (Set)
import qualified Data.Set as Set
import Halay
import Progred.Document
import Progred.Graph
import Progred.MapGraph
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas

renderRawDocument :: Canvas.Canvas renderM => Document -> Point -> renderM Double
renderRawDocument Document {documentRoot, documentGraph} point =
  renderRawGraph (mapGraph documentGraph) documentRoot point

renderRawGraph :: Canvas.Canvas renderM => Graph -> UUID -> Point -> renderM Double
renderRawGraph graph root point = do
  (size, ()) <- placeAt unconstrained point (rawNode graph Set.empty root)
  pure (pointY point + sizeHeight size)

rawDocument :: Canvas.Canvas renderM => Document -> Halay renderM ()
rawDocument Document {documentRoot, documentGraph} =
  rawNode (mapGraph documentGraph) Set.empty documentRoot

rawNode :: Canvas.Canvas renderM => Graph -> Set UUID -> UUID -> Halay renderM ()
rawNode graph visited uuid =
  if Set.member uuid visited
    then rowWithGap 8 [identiconPlay uuid, textPlay "#8a5a00" "..."]
    else case graph uuid of
      Nothing -> rowWithGap 8 [identiconPlay uuid, textPlay "#9a2d2d" "<missing>"]
      Just edges ->
        column
          [ identiconPlay uuid
          , box rawIndentBox [rawEdges graph (Set.insert uuid visited) (toList edges)]
          ]

rawEdges :: Canvas.Canvas renderM => Graph -> Set UUID -> [(UUID, Value)] -> Halay renderM ()
rawEdges graph visited =
  column . fmap rawEdge
  where
    rawEdge (label, value) =
      rowWithGap valueGap [rawEdgeLabel label, rawValue graph visited value]

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM ()
rawEdgeLabel label =
  rowWithGap arrowGap [identiconPlay label, arrowPlay]

rawValue :: Canvas.Canvas renderM => Graph -> Set UUID -> Value -> Halay renderM ()
rawValue graph visited value =
  case value of
    VRef uuid -> rawNode graph visited uuid
    VString string -> textPlay "#20242a" (show string)
    VInt integer -> textPlay "#365f9f" (show integer)
    VFloat double -> textPlay "#365f9f" (show double)
    VBool bool -> textPlay "#7a3fa0" (if bool then "true" else "false")
    VList values -> rawList graph visited values

rawList :: Canvas.Canvas renderM => Graph -> Set UUID -> [Value] -> Halay renderM ()
rawList _ _ [] =
  textPlay "#68707c" "[]"
rawList graph visited values =
  column
    [ textPlay "#68707c" "["
    , box rawIndentBox [column (rawValue graph visited <$> values)]
    , textPlay "#68707c" "]"
    ]

rawIndentBox :: BoxConfig
rawIndentBox =
  defaultBox
    { boxDirection = TopToBottom
    , boxPadding = Insets 0 0 0 indent
    , boxWidth = Fit
    , boxHeight = Fit
    }

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM ()
identiconPlay uuid =
  leaf (pure (Size iconSize lineHeight)) draw
  where
    draw Rect {x, y} =
      identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM ()
textPlay color string =
  text config string
  where
    config =
      TextConfig
        { textLineHeight = Just lineHeight
        , textWrapMode = TextWrapWords
        , textAlign = TextAlignStart
        , textMeasure = \line -> Size <$> Canvas.measureText line <*> pure lineHeight
        , textPlaceLine = \_lineIndex line Rect {x, y} ->
            Canvas.fillText (Point x (y + textBaseline)) color line
        }

arrowPlay :: Canvas.Canvas renderM => Halay renderM ()
arrowPlay =
  leaf (pure (Size arrowWidth lineHeight)) draw
  where
    draw Rect {x, y} =
      drawArrow (Point x (y + iconSize / 2))

iconSize :: Double
iconSize = 20

lineHeight :: Double
lineHeight = 26

textBaseline :: Double
textBaseline = 16

indent :: Double
indent = 28

drawArrow :: Canvas.Canvas renderM => Point -> renderM ()
drawArrow Point {pointX, pointY} = do
  Canvas.fillRect (Rect pointX pointY arrowStemWidth 1) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth) (pointY - 2) 1 5) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth + 1) (pointY - 1) 1 3) arrowColor
  Canvas.fillRect (Rect (pointX + arrowStemWidth + 2) pointY 1 1) arrowColor

arrowColor :: String
arrowColor = "#68707c"

arrowStemWidth :: Double
arrowStemWidth = 10

arrowWidth :: Double
arrowWidth = 13

arrowGap :: Double
arrowGap = 6

valueGap :: Double
valueGap = 10
