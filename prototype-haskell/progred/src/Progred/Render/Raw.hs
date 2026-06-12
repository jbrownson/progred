module Progred.Render.Raw
  ( Focus (..)
  , FocusCursor (..)
  , RawEnv (..)
  , rawDocument
  , transportFocus
  ) where

import Data.Map.Strict (toList)
import qualified Data.Map.Strict as Map
import Data.Maybe (isJust)
import Data.Set (Set)
import qualified Data.Set as Set
import Halay
import Progred.Document
import Progred.Graph
import Progred.MapGraph
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas
import Puri.Handler
import Puri.Widgets.Frame
import Puri.Widgets.LineEdit

-- Focus is a selection chain through the projection tree: a path of
-- edge labels from the document root. Descent peels it apart (a spot is
-- focused when the remainder reaches it), and installing focus rebuilds
-- the chain through the installers each step wraps on the way down. One
-- chain in the model means exactly one focus, and occurrences of a
-- shared node are distinct because they are reached along different
-- paths.
data Focus
  = FocusEdge UUID Focus
  | FocusText EditView
  deriving (Show)

data FocusCursor actionM = FocusCursor
  { focusHere :: Maybe Focus
  , installFocus :: Focus -> actionM ()
  }

edgeCursor :: UUID -> FocusCursor actionM -> FocusCursor actionM
edgeCursor label cursor =
  FocusCursor
    { focusHere =
        case focusHere cursor of
          Just (FocusEdge step rest) | step == label -> Just rest
          _ -> Nothing
    , installFocus = installFocus cursor . FocusEdge label
    }

data RawEnv actionM = RawEnv
  { rawApplyEdit :: MapGraphDelta -> actionM ()
  , rawClearFocus :: actionM ()
  }

-- The structure derivative of this projection: maps a focus chain valid
-- against the old graph to one valid after the delta, walking the chain
-- with the old graph to follow refs. Touching an edge on the chain's
-- spine (set or delete) kills the chain at or below it; handlers that
-- edit the focused value reinstall focus after applying their delta.
-- No cycle bookkeeping is needed: a delta can only change "..." spots on
-- the chain's own spine by editing a spine edge, which already kills it.
transportFocus :: MapGraph -> UUID -> MapGraphDelta -> Focus -> Maybe Focus
transportFocus graph root (MapGraphDelta delta) =
  goNode root
  where
    goNode node focus =
      case focus of
        FocusText _ -> Just focus
        FocusEdge label rest -> do
          edges <- Map.lookup node graph
          value <- Map.lookup label edges
          let nodeDelta = Map.lookup node delta
          if maybe False nodeDeltaResets nodeDelta || Map.member label (maybe Map.empty nodeDeltaEdges nodeDelta)
            then Nothing
            else FocusEdge label <$> goValue value rest
    goValue value focus =
      case value of
        VRef target -> goNode target focus
        _ -> Just focus

rawDocument :: (Applicative actionM, Canvas.Canvas renderM) => RawEnv actionM -> FocusCursor actionM -> Document -> Halay renderM (Handler actionM)
rawDocument env cursor Document {documentRoot, documentGraph} =
  rawNode env cursor (mapGraph documentGraph) Set.empty documentRoot

rawNode :: (Applicative actionM, Canvas.Canvas renderM) => RawEnv actionM -> FocusCursor actionM -> Graph -> Set UUID -> UUID -> Halay renderM (Handler actionM)
rawNode env cursor graph visited uuid =
  if Set.member uuid visited
    then rowWithGap 8 [identiconPlay uuid, textPlay "#8a5a00" "..."]
    else case graph uuid of
      Nothing -> rowWithGap 8 [identiconPlay uuid, textPlay "#9a2d2d" "<missing>"]
      Just edges ->
        column
          [ identiconPlay uuid
          , box rawIndentBox [rawEdges env cursor graph (Set.insert uuid visited) uuid (toList edges)]
          ]

rawEdges :: (Applicative actionM, Canvas.Canvas renderM) => RawEnv actionM -> FocusCursor actionM -> Graph -> Set UUID -> UUID -> [(UUID, Value)] -> Halay renderM (Handler actionM)
rawEdges env cursor graph visited source edges =
  column (rawEdge <$> edges)
  where
    rawEdge (label, value) =
      rowWithGap
        valueGap
        [ rawEdgeLabel label
        , rawValue env (edgeCursor label cursor) (rawApplyEdit env . setEdgeDelta source label) graph visited value
        ]

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM (Handler actionM)
rawEdgeLabel label =
  rowWithGap arrowGap [identiconPlay label, arrowPlay]

rawValue :: (Applicative actionM, Canvas.Canvas renderM) => RawEnv actionM -> FocusCursor actionM -> (Value -> actionM ()) -> Graph -> Set UUID -> Value -> Halay renderM (Handler actionM)
rawValue env cursor setValue graph visited value =
  case value of
    VRef uuid -> rawNode env cursor graph visited uuid
    VString string -> stringBox env cursor (setValue . VString) string
    VInt integer -> textPlay numberColor (show integer)
    VFloat double -> textPlay numberColor (show double)
    VBool bool -> textPlay boolColor (if bool then "true" else "false")

stringBox :: (Applicative actionM, Canvas.Canvas renderM) => RawEnv actionM -> FocusCursor actionM -> (String -> actionM ()) -> String -> Halay renderM (Handler actionM)
stringBox env cursor setString string =
  framed (stringFrame (isJust view)) (lineEdit stringLineStyle string view change)
  where
    view =
      case focusHere cursor of
        Just (FocusText editView) -> Just editView
        _ -> Nothing
    change newString newView =
      setString newString *> maybe (rawClearFocus env) (installFocus cursor . FocusText) newView

stringFrame :: Bool -> Frame
stringFrame focused =
  Frame
    { framePadding = Insets 0 0 0 0
    , frameInsets = Insets 0 0 boxBottomGap 0
    , frameColor = if focused then focusColor else boxBorderColor
    }

stringLineStyle :: LineStyle
stringLineStyle =
  LineStyle
    { lineHeight = rowHeight
    , lineBaseline = textBaseline
    , lineAscent = textAscent
    , lineDescent = textDescent
    , linePadding = boxPad
    , lineMinWidth = minBoxTextWidth
    , lineTextColor = stringColor
    , lineCaretColor = focusColor
    , lineSelectionColor = selectionColor
    }

rawIndentBox :: BoxConfig
rawIndentBox =
  defaultBox
    { boxDirection = TopToBottom
    , boxPadding = Insets 0 0 0 indent
    }

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM (Handler actionM)
identiconPlay uuid =
  leaf (pure (Size iconSize rowHeight)) draw
  where
    draw Rect {x, y} =
      mempty <$ identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM (Handler actionM)
textPlay color string =
  text config string
  where
    config =
      TextConfig
        { textLineHeight = Just rowHeight
        , textWrapMode = TextWrapWords
        , textAlign = TextAlignStart
        , textMeasure = \line -> Size <$> Canvas.measureText line <*> pure rowHeight
        , textPlaceLine = \_lineIndex line Rect {x, y} ->
            mempty <$ Canvas.fillText (Point x (y + textBaseline)) color line
        }

arrowPlay :: Canvas.Canvas renderM => Halay renderM (Handler actionM)
arrowPlay =
  leaf (pure (Size arrowWidth rowHeight)) draw
  where
    draw Rect {x, y} =
      mempty <$ drawArrow (Point x (y + iconSize / 2))

iconSize :: Double
iconSize = 20

rowHeight :: Double
rowHeight = 26

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

stringColor :: String
stringColor = "#20242a"

numberColor :: String
numberColor = "#365f9f"

boolColor :: String
boolColor = "#7a3fa0"

focusColor :: String
focusColor = "#0a84ff"

boxBorderColor :: String
boxBorderColor = "#c8ccd2"

boxPad :: Double
boxPad = 5

boxBottomGap :: Double
boxBottomGap = 4

textAscent :: Double
textAscent = 12

textDescent :: Double
textDescent = 2

selectionColor :: String
selectionColor = "#cfe3ff"

minBoxTextWidth :: Double
minBoxTextWidth = 6

arrowStemWidth :: Double
arrowStemWidth = 10

arrowWidth :: Double
arrowWidth = 13

arrowGap :: Double
arrowGap = 6

valueGap :: Double
valueGap = 10
