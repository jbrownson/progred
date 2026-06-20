module Progred.Render.Raw
  ( focusedProjection
  , inlineRowWithGap
  , rawProjection
  , textPlay
  ) where

import qualified Data.Map.Strict as Map
import Halay
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.Projection
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas
import Puri.Halay (lineEdit)
import Puri.Handler
import qualified Puri.KeyCode as KeyCode
import Puri.Widgets (LineEditInteraction (..), LineEditSelection, LineStyle (..), emptyLineEditSelection, lineEditSelectionAtEnd)
import Puri.Widgets.Frame

-- The total projection at the bottom of every composition: assumes
-- nothing, renders whatever the spot holds, placeholders included.
rawProjection :: (Canvas.Canvas renderM, Monad actionM) => Projection actionM renderM
rawProjection env cursor =
  case resolveCursor env cursor of
    Nothing
      | null (cursorPath cursor) -> rootPlaceholder env cursor
      | otherwise -> textPlay missingColor "<missing>"
    Just resolved -> rawValue env resolved

focusedProjection :: Canvas.Canvas renderM => Projection actionM renderM -> Projection actionM renderM
focusedProjection projection env cursor =
  secondaryCursor env cursor (focusCursor env cursor (projection env cursor))

focusCursor :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
focusCursor env cursor child =
  case cursorFocus cursor of
    Just focus | null (focusPath focus) && focusPendingEdit (focusState focus) == Nothing && shouldDrawFocusBackground env cursor ->
      decorate drawFocusBackground child
    _ -> child

secondaryCursor :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
secondaryCursor env cursor child =
  case (envSecondaryHighlight env, cursorFocus cursor) of
    (Just highlight, _)
      | not (spotHasPrimaryFocus cursor)
      , secondaryMatches highlight env cursor ->
          decorate drawSecondaryBackground child
    _ -> child
  where
    secondaryMatches highlight spotEnv spot =
      case highlight of
        SecondaryNode uuid ->
          case resolveCursor spotEnv spot of
            Just resolved -> resolvedValue resolved == VRef uuid
            Nothing -> False
        SecondarySpot path ->
          cursorPath spot == path
    spotHasPrimaryFocus spot =
      case cursorFocus spot of
        Just focus -> null (focusPath focus) && focusPendingEdit (focusState focus) == Nothing
        Nothing -> False

drawSecondaryBackground :: Canvas.Canvas renderM => Rect -> renderM (Handler actionM)
drawSecondaryBackground rect = do
  Canvas.fillRect rect secondaryFocusBackgroundColor
  Canvas.strokeRect rect secondaryFocusColor 1
  pure mempty

shouldDrawFocusBackground :: Env actionM renderM -> Cursor -> Bool
shouldDrawFocusBackground env cursor =
  case resolvedValue <$> resolveCursor env cursor of
    Just (VString _) -> False
    Just (VInt _) -> False
    Just (VFloat _) -> False
    _ -> True

drawFocusBackground :: Canvas.Canvas renderM => Rect -> renderM (Handler actionM)
drawFocusBackground rect = do
  Canvas.fillRect rect focusBackgroundColor
  Canvas.strokeRect rect focusColor 1
  pure mempty

rawValue :: (Canvas.Canvas renderM, Monad actionM) => Env actionM renderM -> ResolvedCursor -> Halay renderM renderM (Handler actionM)
rawValue env resolved =
  case resolvedValue resolved of
    VRef target -> rawNode env (resolvedCursor resolved) target (target `elem` resolvedNodes resolved)
    VString string -> stringBox env cursor string
    VInt integer -> numberBox env cursor (show integer) parseIntValue editInt
    VFloat double -> numberBox env cursor (show double) parseFloatValue editFloat
  where
    cursor = resolvedCursor resolved

rawNode :: (Canvas.Canvas renderM, Monad actionM) => Env actionM renderM -> Cursor -> UUID -> Bool -> Halay renderM renderM (Handler actionM)
rawNode env cursor target collapsedByDefault =
  case lookupNode (envContext env) target of
    Nothing -> nodeReferenceActions env target (inlineRowWithGap valueGap [identiconPlay target, textPlay missingColor "<missing>"])
    Just edges ->
      nodeReferenceActions env target $
        rootActions env cursor $
          rawNodeActions env cursor $
            if visibleCollapsed
              then rawNodeHeader env cursor target edges visibleCollapsed
              else
                column
                  [ rawNodeHeader env cursor target edges visibleCollapsed
                  , box rawIndentBox [column ((rawEdge <$> Map.toList edges) <> pendingRows)]
                  ]
  where
    collapsed =
      case envCollapseState env (cursorPath cursor) of
        Just explicit -> explicit
        Nothing -> collapsedByDefault
    activeRawPending = activePending cursor
    visibleCollapsed = collapsed && activeRawPending == Nothing
    rawEdge (label, _value) =
      edgeRow env cursor label
    pendingRows =
      case activeRawPending of
        Just (label, pending) -> [pendingEdgeRow env cursor label pending]
        Nothing -> []

rawNodeHeader :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> Edges -> Bool -> Halay renderM renderM (Handler actionM)
rawNodeHeader env cursor target edges displayedCollapsed
  | Map.null edges = identiconPlay target
  | otherwise =
      inlineRowWithGap
        collapseHeaderGap
        [ identiconPlay target
        , collapseToggle env cursor displayedCollapsed
        ]

nodeReferenceActions
  :: (Applicative renderM, Monad actionM)
  => Env actionM renderM
  -> UUID
  -> Halay renderM renderM (Handler actionM)
  -> Halay renderM renderM (Handler actionM)
nodeReferenceActions env target child =
  decorate place child
  where
    place rect =
      pure $
        onPointerCapture $ \event ->
          case event of
            PointerDown {pointerX, pointerY, pointerModifiers}
              | keyMeta pointerModifiers && rectContains rect pointerX pointerY ->
                  Just $ do
                    cell <- envFreshUUID env
                    envEdit env (replaceFocusedSpot cell (VRef target))
            _ -> Nothing

rootActions :: Applicative renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
rootActions env cursor child
  | null (cursorPath cursor) = focusableSpot env cursor child
  | otherwise = child

rawNodeActions :: (Applicative renderM, Monad actionM) => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
rawNodeActions env cursor child =
  case cursorFocus cursor of
    Just focus | null (focusPath focus) && focusPendingEdit (focusState focus) == Nothing ->
      decorate (const (pure (onInsert (startPendingEdge env (cursorPath cursor))))) child
    _ -> child

edgeRow
  :: (Canvas.Canvas renderM, Monad actionM)
  => Env actionM renderM
  -> Cursor
  -> UUID
  -> Halay renderM renderM (Handler actionM)
edgeRow env cursor label =
  rawEdgeActions env cursor childCursor $
    focusableEdge env childCursor $
      rowWithGap valueGap [rawEdgeLabel label, rawChild (envProject env childCursor)]
  where
    childCursor = descendCursor label cursor

rawEdgeActions :: (Applicative renderM, Monad actionM) => Env actionM renderM -> Cursor -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
rawEdgeActions env parentCursor childCursor child =
  case cursorFocus childCursor of
    Just focus | null (focusPath focus) && focusPendingEdit (focusState focus) == Nothing ->
      decorate (const (pure (onInsert (startPendingEdge env (cursorPath parentCursor))))) child
    _ -> child

startPendingEdge :: Monad actionM => Env actionM renderM -> [UUID] -> actionM ()
startPendingEdge env parentPath = do
  label <- envFreshUUID env
  envEdit env (focusPending (parentPath <> [label]) "" emptyLineEditSelection . setCollapsed parentPath False)

rawEdgeLabel :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
rawEdgeLabel label =
  inlineRowWithGap arrowGap [identiconPlay label, arrowPlay]

pendingEdgeRow :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> PendingEdit -> Halay renderM renderM (Handler actionM)
pendingEdgeRow env cursor label pending =
  rowWithGap valueGap [rawEdgeLabel label, rawChild (rawPendingInsert env cursor label pending)]

rawChild :: Applicative measureM => Halay measureM placeM placed -> Halay measureM placeM placed
rawChild =
  padding rawChildPadding

collapseToggle :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Bool -> Halay renderM renderM (Handler actionM)
collapseToggle env cursor collapsed =
  leaf (pure (Size collapseToggleWidth iconSize)) draw
  where
    path = cursorPath cursor
    draw rect = do
      drawCollapseToggle collapsed rect
      pure $
        onPointerCapture $ \event ->
          case event of
            PointerDown {pointerX, pointerY}
              | rectContains rect pointerX pointerY ->
                  Just (envEdit env (setCollapsed path (not collapsed)))
            _ -> Nothing

drawCollapseToggle :: Canvas.Canvas renderM => Bool -> Rect -> renderM ()
drawCollapseToggle collapsed rect =
  if collapsed
    then drawRightDisclosure rect
    else drawDownDisclosure rect

drawRightDisclosure :: Canvas.Canvas renderM => Rect -> renderM ()
drawRightDisclosure Rect {x, y, width, height} =
  mapM_ drawStep disclosureSteps
  where
    left = x + (width - disclosureHeight) / 2
    centerY = y + height / 2
    stepWidth = disclosureHeight / fromIntegral disclosureStepCount
    drawStep index =
      Canvas.fillRect (Rect stepX stepY stepWidth stepSpan) collapseColor
      where
        step = fromIntegral index
        progress = (fromIntegral disclosureStepCount - step - 0.5) / fromIntegral disclosureStepCount
        stepSpan = max 1 (disclosureSide * progress)
        stepX = left + step * stepWidth
        stepY = centerY - stepSpan / 2

drawDownDisclosure :: Canvas.Canvas renderM => Rect -> renderM ()
drawDownDisclosure Rect {x, y, width, height} =
  mapM_ drawStep disclosureSteps
  where
    centerX = x + width / 2
    top = y + (height - disclosureHeight) / 2
    stepHeight = disclosureHeight / fromIntegral disclosureStepCount
    drawStep index =
      Canvas.fillRect (Rect stepX stepY stepSpan stepHeight) collapseColor
      where
        step = fromIntegral index
        progress = (fromIntegral disclosureStepCount - step - 0.5) / fromIntegral disclosureStepCount
        stepSpan = max 1 (disclosureSide * progress)
        stepX = centerX - stepSpan / 2
        stepY = top + step * stepHeight

rawPendingInsert :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> UUID -> PendingEdit -> Halay renderM renderM (Handler actionM)
rawPendingInsert env cursor label pending =
  rawPendingText True currentText interaction commit
  where
    parentPath = cursorPath cursor
    path = parentPath <> [label]
    currentText = pendingEditText pending
    selection = pendingEditSelection pending
    cancel = envEdit env (cancelPending path)
    interaction =
      LineEditFocused
        selection
        (\newText newSelection -> envEdit env (focusPending path newText newSelection))
        cancel
    commit =
      envEdit env (insertStringEdge parentPath label currentText (lineEditSelectionAtEnd currentText))

rootPlaceholder :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM)
rootPlaceholder env cursor =
  rawPendingText focused currentText interaction commit
  where
    path = cursorPath cursor
    cancel = envEdit env (cancelPending path)
    commit = envEdit env (editString path currentText (lineEditSelectionAtEnd currentText))
    (focused, currentText, interaction) =
      case cursorFocus cursor of
        Just focus | null (focusPath focus) ->
          let pending = pendingEditOrDefault (focusState focus)
           in ( True
              , pendingEditText pending
              , LineEditFocused
                  (pendingEditSelection pending)
                  (\newText newSelection -> envEdit env (focusPending path newText newSelection))
                  cancel
              )
        _ ->
          ( False
          , ""
          , LineEditUnfocused (\selection -> envEdit env (focusPending path "" selection))
          )

rawPendingText
  :: Canvas.Canvas renderM
  => Bool
  -> String
  -> LineEditInteraction actionM
  -> actionM ()
  -> Halay renderM renderM (Handler actionM)
rawPendingText focused currentText interaction commit =
  framed (rawPendingFrame focused) $
    decorate submitKeys $
      lineEdit rawPendingLineStyle currentText interaction
  where
    submitKeys _rect =
      pure $
        onKey $ \event ->
          case event of
            KeyCode modifiers code
              | code == KeyCode.enter && not (hasModifier modifiers) ->
                  Just commit
            _ -> Nothing

activePending :: Cursor -> Maybe (UUID, PendingEdit)
activePending cursor =
  case cursorFocus cursor of
    Just focus ->
      case (focusPath focus, focusPendingEdit (focusState focus)) of
        ([label], Just pending) -> Just (label, pending)
        _ -> Nothing
    _ -> Nothing

inlineRowWithGap :: Applicative measureM => Double -> [Halay measureM placeM placed] -> Halay measureM placeM placed
inlineRowWithGap gap =
  box defaultBox {boxDirection = LeftToRight, boxGap = gap, boxCrossAlign = CrossCenter}

stringBox :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> String -> Halay renderM renderM (Handler actionM)
stringBox env cursor string =
  framed (stringFrame (isLineEditFocused interaction)) (lineEdit stringLineStyle string interaction)
  where
    path = cursorPath cursor
    interaction =
      case cursorFocus cursor of
        Just focus | null (focusPath focus) ->
          LineEditFocused
            (focusStringSelection (focusState focus))
            (\newString newSelection -> envEdit env (editString path newString newSelection))
            (envEdit env (blurString path))
        _ ->
          LineEditUnfocused (\selection -> envEdit env (focusString path selection))

numberBox
  :: Canvas.Canvas renderM
  => Env actionM renderM
  -> Cursor
  -> String
  -> (String -> Maybe Value)
  -> ([UUID] -> String -> LineEditSelection -> Editor -> Editor)
  -> Halay renderM renderM (Handler actionM)
numberBox env cursor string parse change =
  framed (stringFrame focused) (lineEdit (numberLineStyle (isValidNumber editText)) editText interaction)
  where
    path = cursorPath cursor
    isValidNumber candidate =
      case parse candidate of
        Just _ -> True
        Nothing -> False
    (focused, editText, interaction) =
      case cursorFocus cursor of
        Just focus | null (focusPath focus) ->
          let edit = numberEditOrDefault string (focusState focus)
           in ( True
              , numberEditText edit
              , LineEditFocused
                  (numberEditSelection edit)
                  (\newString newSelection -> envEdit env (change path newString newSelection))
                  (envEdit env (blurValue path))
              )
        _ ->
          ( False
          , string
          , LineEditUnfocused (\selection -> envEdit env (focusNumber path string selection))
          )

numberEditOrDefault :: String -> FocusState -> NumberEdit
numberEditOrDefault string state =
  case focusNumberEdit state of
    Just edit -> edit
    Nothing -> NumberEdit string (lineEditSelectionAtEnd string)

pendingEditOrDefault :: FocusState -> PendingEdit
pendingEditOrDefault state =
  case focusPendingEdit state of
    Just pending -> pending
    Nothing -> PendingEdit "" emptyLineEditSelection

isLineEditFocused :: LineEditInteraction actionM -> Bool
isLineEditFocused interaction =
  case interaction of
    LineEditUnfocused _focus -> False
    LineEditFocused _selection _change _blur -> True

stringFrame :: Bool -> Frame
stringFrame focused =
  Frame
    { framePadding = Insets 0 0 0 0
    , frameInsets = Insets 0 0 0 0
    , frameBackground = if focused then Just focusBackgroundColor else Nothing
    , frameColor = if focused then focusColor else boxBorderColor
    }

stringLineStyle :: LineStyle
stringLineStyle =
  LineStyle
    { lineVerticalPadding = scalarVerticalPadding
    , linePadding = boxPad
    , lineMinWidth = minBoxTextWidth
    , lineTextColor = stringColor
    , lineCaretColor = focusColor
    , lineSelectionColor = selectionColor
    }

numberLineStyle :: Bool -> LineStyle
numberLineStyle valid =
  stringLineStyle
    { lineTextColor = if valid then numberColor else invalidNumberColor
    }

rawPendingFrame :: Bool -> Frame
rawPendingFrame focused =
  (stringFrame focused) {frameBackground = if focused then Just "#fff9e8" else Nothing}

rawPendingLineStyle :: LineStyle
rawPendingLineStyle =
  stringLineStyle {lineMinWidth = 32}

rawIndentBox :: BoxConfig
rawIndentBox =
  defaultBox
    { boxDirection = TopToBottom
    , boxPadding = Insets 0 0 0 indent
    }

rawChildPadding :: Insets
rawChildPadding =
  Insets 2 3 2 3

identiconPlay :: Canvas.Canvas renderM => UUID -> Halay renderM renderM (Handler actionM)
identiconPlay uuid =
  leaf (pure (Size iconSize iconSize)) draw
  where
    draw Rect {x, y} =
      mempty <$ identicon uuid (Rect x y iconSize iconSize)

textPlay :: Canvas.Canvas renderM => String -> String -> Halay renderM renderM (Handler actionM)
textPlay color string =
  text config string
  where
    config =
      TextConfig
        { textLineHeight = Nothing
        , textWrapMode = TextWrapWords
        , textAlign = TextAlignStart
        , textMeasure = measureTextLine
        , textPlaceLine = \_lineIndex line Rect {x, y} -> do
            metrics <- Canvas.measureText textMetricSample
            mempty <$ Canvas.fillText (Point x (y + Canvas.textFontBoundingBoxAscent metrics)) color line
        }
    measureTextLine line = do
      textMetrics <- Canvas.measureText line
      lineMetrics <- Canvas.measureText textMetricSample
      pure (Size (Canvas.textWidth textMetrics) (textMetricHeight lineMetrics))

arrowPlay :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
arrowPlay =
  leaf (pure (Size arrowWidth iconSize)) draw
  where
    draw Rect {x, y} =
      mempty <$ drawArrow (Point x (y + iconSize / 2))

iconSize :: Double
iconSize = 20

collapseToggleWidth :: Double
collapseToggleWidth = 14

collapseHeaderGap :: Double
collapseHeaderGap = 4

disclosureSide :: Double
disclosureSide = 6

disclosureHeight :: Double
disclosureHeight = disclosureSide * sqrt 3 / 2

disclosureStepCount :: Int
disclosureStepCount = 6

disclosureSteps :: [Int]
disclosureSteps =
  [0 .. disclosureStepCount - 1]

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

missingColor :: String
missingColor = "#9a2d2d"

invalidNumberColor :: String
invalidNumberColor = "#b42318"

collapseColor :: String
collapseColor = "#68707c"

focusColor :: String
focusColor = "#0a84ff"

focusBackgroundColor :: String
focusBackgroundColor = "#eaf3ff"

secondaryFocusColor :: String
secondaryFocusColor = "#777777"

secondaryFocusBackgroundColor :: String
secondaryFocusBackgroundColor = "#f3f3f3"

boxBorderColor :: String
boxBorderColor = "#c8ccd2"

boxPad :: Double
boxPad = 5

scalarVerticalPadding :: Double
scalarVerticalPadding = 2

textMetricSample :: String
textMetricSample = "Mg"

textMetricHeight :: Canvas.TextMetrics -> Double
textMetricHeight metrics =
  Canvas.textFontBoundingBoxAscent metrics + Canvas.textFontBoundingBoxDescent metrics

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
