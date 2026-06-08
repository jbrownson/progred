module Halay
  ( AxisSizing (..)
  , BoxConfig (..)
  , BoxClip (..)
  , CrossAlign (..)
  , Direction (..)
  , Halay
  , MainAlign (..)
  , Measured
  , Sizing (..)
  , TextAlign (..)
  , TextConfig (..)
  , TextWrapMode (..)
  , aspectRatio
  , box
  , column
  , columnWithGap
  , decorate
  , defaultBox
  , empty
  , fixed
  , leaf
  , leafWithSizing
  , measureHalay
  , padding
  , placeAt
  , placeHalay
  , row
  , rowWithGap
  , sized
  , text
  , module Halay.Geometry
  ) where

import Control.Monad (foldM)
import Halay.Geometry

data AxisSizing
  = Fit
  | Fixed Double
  | Fill
  | Percent Double
  | Clamp (Maybe Double) (Maybe Double) AxisSizing
  deriving (Eq, Show)

data Sizing = Sizing
  { sizingWidth :: AxisSizing
  , sizingHeight :: AxisSizing
  }
  deriving (Eq, Show)

data Direction
  = LeftToRight
  | TopToBottom
  deriving (Eq, Show)

data MainAlign
  = MainStart
  | MainCenter
  | MainEnd
  deriving (Eq, Show)

data CrossAlign
  = CrossStart
  | CrossCenter
  | CrossEnd
  deriving (Eq, Show)

data BoxConfig = BoxConfig
  { boxDirection :: Direction
  , boxPadding :: Insets
  , boxGap :: Double
  , boxWidth :: AxisSizing
  , boxHeight :: AxisSizing
  , boxMainAlign :: MainAlign
  , boxCrossAlign :: CrossAlign
  , boxClip :: BoxClip
  }
  deriving (Eq, Show)

data BoxClip = BoxClip
  { clipHorizontal :: Bool
  , clipVertical :: Bool
  , clipChildOffset :: Point
  }
  deriving (Eq, Show)

data TextWrapMode
  = TextWrapWords
  | TextWrapNewlines
  | TextWrapNone
  deriving (Eq, Show)

data TextAlign
  = TextAlignStart
  | TextAlignCenter
  | TextAlignEnd
  deriving (Eq, Show)

data TextConfig measureM placed = TextConfig
  { textLineHeight :: Maybe Double
  , textWrapMode :: TextWrapMode
  , textAlign :: TextAlign
  , textMeasure :: String -> measureM Size
  , textPlaceLine :: Int -> String -> Rect -> measureM placed
  }

newtype Halay measureM placed = Halay
  { buildHalay :: measureM (LayoutNode measureM placed)
  }

data Measured measureM placed = Measured
  { measuredSize :: Size
  , measuredMinSize :: Size
  , measuredSizing :: Sizing
  , measuredAspectRatio :: Maybe Double
  , placeMeasured :: Rect -> measureM placed
  }

data LayoutNode measureM placed = LayoutNode
  { nodeConfig :: BoxConfig
  , nodeAspectRatio :: Maybe Double
  , nodeHeightMaxOverride :: Maybe Double
  , nodeIntrinsicSize :: Maybe Size
  , nodeText :: Maybe (TextNode measureM placed)
  , nodePlacers :: [Rect -> measureM placed]
  , nodeChildren :: [LayoutNode measureM placed]
  , nodeDimensions :: Size
  , nodeMinDimensions :: Size
  }

data TextNode measureM placed = TextNode
  { textNodeText :: String
  , textNodeConfig :: TextConfig measureM placed
  , textNodePreferredSize :: Size
  , textNodeMinWidth :: Double
  , textNodeSpaceWidth :: Double
  , textNodeTokens :: [TextToken]
  , textNodeContainsNewlines :: Bool
  , textNodeLines :: [TextLine]
  }

data TextToken
  = TextWord String Double
  | TextNewline

data TextLine = TextLine
  { lineText :: String
  , lineWidth :: Double
  }

defaultSizing :: Sizing
defaultSizing = Sizing Fit Fit

defaultBox :: BoxConfig
defaultBox =
  BoxConfig
    { boxDirection = LeftToRight
    , boxPadding = Insets 0 0 0 0
    , boxGap = 0
    , boxWidth = Fit
    , boxHeight = Fit
    , boxMainAlign = MainStart
    , boxCrossAlign = CrossStart
    , boxClip = BoxClip False False (Point 0 0)
    }

empty :: (Applicative measureM, Monoid placed) => Halay measureM placed
empty =
  fixed (Size 0 0) mempty

fixed :: Applicative measureM => Size -> placed -> Halay measureM placed
fixed size placed =
  leafWithSizing (Sizing (Fixed (sizeWidth size)) (Fixed (sizeHeight size))) (pure size) (const (pure placed))

leaf :: Applicative measureM => measureM Size -> (Rect -> measureM placed) -> Halay measureM placed
leaf =
  leafWithSizing defaultSizing

leafWithSizing :: Applicative measureM => Sizing -> measureM Size -> (Rect -> measureM placed) -> Halay measureM placed
leafWithSizing sizing measure place =
  Halay $
    makeNode <$> measure
  where
    makeNode intrinsicSize =
      emptyNode
        { nodeConfig = defaultBox {boxWidth = sizingWidth sizing, boxHeight = sizingHeight sizing}
        , nodeIntrinsicSize = Just intrinsicSize
        , nodePlacers = [place]
        }

text :: Monad measureM => TextConfig measureM placed -> String -> Halay measureM placed
text config string =
  Halay $ do
    measured <- measureTextNode config string
    pure
      emptyNode
        { nodeText = Just measured
        , nodeDimensions = textNodePreferredSize measured
        , nodeMinDimensions =
            Size
              { sizeWidth = textNodeMinWidth measured
              , sizeHeight = textNodeLineHeight measured
              }
        }

sized :: Functor measureM => Sizing -> Halay measureM placed -> Halay measureM placed
sized sizing child =
  Halay $
    setNodeSizing sizing <$> buildHalay child

decorate :: Functor measureM => (Rect -> measureM placed) -> Halay measureM placed -> Halay measureM placed
decorate place child =
  Halay $
    addNodePlacer place <$> buildHalay child

aspectRatio :: Functor measureM => Double -> Halay measureM placed -> Halay measureM placed
aspectRatio ratio child =
  Halay $
    setNodeAspectRatio ratio <$> buildHalay child

placeHalay :: (Monad measureM, Monoid placed) => Constraints -> Rect -> Halay measureM placed -> measureM placed
placeHalay constraints rect halay = do
  measured <- measureHalay halay constraints
  placeMeasured measured rect

placeAt :: (Monad measureM, Monoid placed) => Constraints -> Point -> Halay measureM placed -> measureM (Size, placed)
placeAt constraints point halay = do
  measured <- measureHalay halay constraints
  placed <- placeMeasured measured (sizeRectAt point (measuredSize measured))
  pure (measuredSize measured, placed)

measureHalay :: (Monad measureM, Monoid placed) => Halay measureM placed -> Constraints -> measureM (Measured measureM placed)
measureHalay halay _constraints = do
  source <- buildHalay halay
  let measuredNode = layoutNode Nothing source
  pure
    Measured
      { measuredSize = nodeDimensions measuredNode
      , measuredMinSize = nodeMinDimensions measuredNode
      , measuredSizing = nodeSizing measuredNode
      , measuredAspectRatio = nodeAspectRatio measuredNode
      , placeMeasured = \rect ->
          placeLayoutNode (Point (x rect) (y rect)) (layoutNode (Just (Size (width rect) (height rect))) source)
      }

row :: Monad measureM => [Halay measureM placed] -> Halay measureM placed
row =
  rowWithGap 0

rowWithGap :: Monad measureM => Double -> [Halay measureM placed] -> Halay measureM placed
rowWithGap gap =
  box defaultBox {boxDirection = LeftToRight, boxGap = gap}

column :: Monad measureM => [Halay measureM placed] -> Halay measureM placed
column =
  columnWithGap 0

columnWithGap :: Monad measureM => Double -> [Halay measureM placed] -> Halay measureM placed
columnWithGap gap =
  box defaultBox {boxDirection = TopToBottom, boxGap = gap}

padding :: Monad measureM => Insets -> Halay measureM placed -> Halay measureM placed
padding insets child =
  box defaultBox {boxPadding = insets} [child]

box :: Monad measureM => BoxConfig -> [Halay measureM placed] -> Halay measureM placed
box config children =
  Halay $ do
    childNodes <- traverse buildHalay children
    pure emptyNode {nodeConfig = config, nodeChildren = childNodes}

emptyNode :: LayoutNode measureM placed
emptyNode =
  LayoutNode
    { nodeConfig = defaultBox
    , nodeAspectRatio = Nothing
    , nodeHeightMaxOverride = Nothing
    , nodeIntrinsicSize = Nothing
    , nodeText = Nothing
    , nodePlacers = []
    , nodeChildren = []
    , nodeDimensions = Size 0 0
    , nodeMinDimensions = Size 0 0
    }

setNodeSizing :: Sizing -> LayoutNode measureM placed -> LayoutNode measureM placed
setNodeSizing Sizing {sizingWidth, sizingHeight} node =
  node {nodeConfig = (nodeConfig node) {boxWidth = sizingWidth, boxHeight = sizingHeight}}

setNodeAspectRatio :: Double -> LayoutNode measureM placed -> LayoutNode measureM placed
setNodeAspectRatio ratio node =
  node {nodeAspectRatio = Just ratio}

addNodePlacer :: (Rect -> measureM placed) -> LayoutNode measureM placed -> LayoutNode measureM placed
addNodePlacer place node =
  node {nodePlacers = place : nodePlacers node}

nodeSizing :: LayoutNode measureM placed -> Sizing
nodeSizing LayoutNode {nodeConfig = BoxConfig {boxWidth, boxHeight}} =
  Sizing boxWidth boxHeight

layoutNode :: Maybe Size -> LayoutNode measureM placed -> LayoutNode measureM placed
layoutNode rootSizeOverride =
  scaleAspectWidths
    . sizeContainersAlongAxis Vertical
    . propagateResolvedHeights
    . wrapTextNodes
    . scaleAspectHeights
    . sizeContainersAlongAxis Horizontal
    . overrideRootSize rootSizeOverride
    . closeNode

overrideRootSize :: Maybe Size -> LayoutNode measureM placed -> LayoutNode measureM placed
overrideRootSize Nothing node = node
overrideRootSize (Just size) node = node {nodeDimensions = size}

closeNode :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNode node =
  closeNodeAspect (closeNodeSizing closed)
  where
    closedChildren = closeNode <$> nodeChildren node
    closed =
      case nodeText node of
        Just textNode ->
          node
            { nodeChildren = closedChildren
            , nodeDimensions = textNodePreferredSize textNode
            , nodeMinDimensions =
                Size
                  { sizeWidth = textNodeMinWidth textNode
                  , sizeHeight = textNodeLineHeight textNode
                  }
            }
        Nothing ->
          case nodeIntrinsicSize node of
            Just intrinsicSize ->
              node
                { nodeChildren = closedChildren
                , nodeDimensions = intrinsicSize
                , nodeMinDimensions = intrinsicSize
                }
            Nothing ->
              closeContainer node {nodeChildren = closedChildren}

closeContainer :: LayoutNode measureM placed -> LayoutNode measureM placed
closeContainer node =
  node
    { nodeDimensions = expandSize boxInsets contentSize
    , nodeMinDimensions = closeContainerMinDimensions config minContentSize
    }
  where
    config = nodeConfig node
    children = nodeChildren node
    boxInsets = boxPadding config
    childSizes = nodeDimensions <$> children
    childMinSizes = nodeMinDimensions <$> children
    contentSize =
      case boxDirection config of
        LeftToRight ->
          Size
            { sizeWidth = sum (sizeWidth <$> childSizes) + gapSize (boxGap config) children
            , sizeHeight = maximumOrZero (sizeHeight <$> childSizes)
            }
        TopToBottom ->
          Size
            { sizeWidth = maximumOrZero (sizeWidth <$> childSizes)
            , sizeHeight = sum (sizeHeight <$> childSizes) + gapSize (boxGap config) children
            }
    minContentSize =
      case boxDirection config of
        LeftToRight ->
          Size
            { sizeWidth = sum (sizeWidth <$> childMinSizes) + gapSize (boxGap config) children
            , sizeHeight = maximumOrZero (sizeHeight <$> childMinSizes)
            }
        TopToBottom ->
          Size
            { sizeWidth = maximumOrZero (sizeWidth <$> childMinSizes)
            , sizeHeight = sum (sizeHeight <$> childMinSizes) + gapSize (boxGap config) children
            }

closeContainerMinDimensions :: BoxConfig -> Size -> Size
closeContainerMinDimensions config Size {sizeWidth, sizeHeight} =
  case boxDirection config of
    LeftToRight ->
      Size
        { sizeWidth =
            if clipHorizontal (boxClip config)
              then insetLeft boxInsets + insetRight boxInsets
              else sizeWidth + insetLeft boxInsets + insetRight boxInsets
        , sizeHeight =
            if clipVertical (boxClip config)
              then 0
              else sizeHeight + insetTop boxInsets + insetBottom boxInsets
        }
    TopToBottom ->
      Size
        { sizeWidth =
            if clipHorizontal (boxClip config)
              then 0
              else sizeWidth + insetLeft boxInsets + insetRight boxInsets
        , sizeHeight =
            if clipVertical (boxClip config)
              then insetTop boxInsets + insetBottom boxInsets
              else sizeHeight + insetTop boxInsets + insetBottom boxInsets
        }
  where
    boxInsets = boxPadding config

closeNodeSizing :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNodeSizing node =
  node
    { nodeDimensions =
        Size
          { sizeWidth = closeAxisSize (boxWidth config) (sizeWidth dimensions)
          , sizeHeight = closeAxisSize (boxHeight config) (sizeHeight dimensions)
          }
    , nodeMinDimensions =
        Size
          { sizeWidth = closeAxisMinSize (boxWidth config) (sizeWidth minDimensions)
          , sizeHeight = closeAxisMinSize (boxHeight config) (sizeHeight minDimensions)
          }
    }
  where
    config = nodeConfig node
    dimensions = nodeDimensions node
    minDimensions = nodeMinDimensions node

closeNodeAspect :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNodeAspect =
  updateMissingAspectDimension

updateMissingAspectDimension :: LayoutNode measureM placed -> LayoutNode measureM placed
updateMissingAspectDimension node =
  case nodeAspectRatio node of
    Just ratio
      | ratio /= 0 ->
          node {nodeDimensions = fillMissingAspectDimension ratio (nodeDimensions node)}
    _ -> node

sizeContainersAlongAxis :: Axis -> LayoutNode measureM placed -> LayoutNode measureM placed
sizeContainersAlongAxis axis node =
  node {nodeChildren = sizeContainersAlongAxis axis <$> sizedChildren}
  where
    sizedChildren = resolveAxisChildren axis node

resolveAxisChildren :: Axis -> LayoutNode measureM placed -> [LayoutNode measureM placed]
resolveAxisChildren axis parent =
  if sizingAlongAxis
    then resolveMainAxisChildren axis parent percentChildren
    else resolveCrossAxisChildren axis parent innerContentSize percentChildren
  where
    config = nodeConfig parent
    children = nodeChildren parent
    parentSize = axisSize axis (nodeDimensions parent)
    parentPadding = axisPadding axis (boxPadding config)
    sizingAlongAxis = axisIsAlongDirection axis (boxDirection config)
    gapTotal
      | sizingAlongAxis = gapSize (boxGap config) children
      | otherwise = 0
    totalPaddingAndChildGaps = parentPadding + gapTotal
    innerContentSize =
      if sizingAlongAxis
        then sum [axisSize axis (nodeDimensions child) | child <- children, not (isPercent (axisSizing axis (nodeSizing child)))] + gapTotal
        else maximumOrZero (axisSize axis . nodeDimensions <$> children)
    percentChildren =
      [ case axisSizing axis (nodeSizing child) of
          Percent percent -> updateMissingAspectDimension (updateNodeAxisDimension axis ((parentSize - totalPaddingAndChildGaps) * percent) child)
          _ -> child
      | child <- children
      ]
resolveMainAxisChildren :: Axis -> LayoutNode measureM placed -> [LayoutNode measureM placed] -> [LayoutNode measureM placed]
resolveMainAxisChildren axis parent children
  | sizeToDistribute < 0 && not (nodeClipsAxis axis parent) =
      distributeCompressNodes axis sizeToDistribute baseChildren resizableIndices
  | sizeToDistribute > 0 && not (null growIndices) =
      distributeGrowNodes axis sizeToDistribute baseChildren growIndices
  | otherwise = baseChildren
  where
    config = nodeConfig parent
    parentSize = axisSize axis (nodeDimensions parent)
    parentPadding = axisPadding axis (boxPadding config)
    baseChildren = children
    baseInnerContentSize =
      sum (axisSize axis . nodeDimensions <$> baseChildren)
        + gapSize (boxGap config) baseChildren
    sizeToDistribute = parentSize - parentPadding - baseInnerContentSize
    resizableIndices =
      [index | (index, child) <- zip [0 ..] children, nodeCanResizeAlongAxis axis child]
    growIndices =
      [index | (index, child) <- zip [0 ..] children, isFill (axisSizing axis (nodeSizing child))]

resolveCrossAxisChildren :: Axis -> LayoutNode measureM placed -> Double -> [LayoutNode measureM placed] -> [LayoutNode measureM placed]
resolveCrossAxisChildren axis parent innerContentSize children =
  [ resolve child | child <- children ]
  where
    config = nodeConfig parent
    parentSize = axisSize axis (nodeDimensions parent)
    parentPadding = axisPadding axis (boxPadding config)
    maxSize
      | nodeClipsAxis axis parent = max visibleMaxSize innerContentSize
      | otherwise = visibleMaxSize
    visibleMaxSize = parentSize - parentPadding
    resolve child
      | not (nodeCanResizeAlongAxis axis child) =
          child
      | isFill sizing =
          updateNodeAxisDimension axis (max minSize (min (min maxSize (nodeAxisMax axis child)) maxSize)) child
      | otherwise =
          updateNodeAxisDimension axis (max minSize (min size maxSize)) child
      where
        sizing = axisSizing axis (nodeSizing child)
        size = axisSize axis (nodeDimensions child)
        minSize = axisSize axis (nodeMinDimensions child)

clampNodeHeight :: LayoutNode measureM placed -> Double -> Double
clampNodeHeight node value =
  clampMax (nodeAxisMax Vertical node) (clampMin (nodeHeightMinForPropagation node) value)
  where
    clampMin minimumValue = max minimumValue
    clampMax maximumValue = min maximumValue

nodeHeightMinForPropagation :: LayoutNode measureM placed -> Double
nodeHeightMinForPropagation node =
  case percentValue sizing of
    Just percent -> percent
    Nothing -> axisMin sizing
  where
    sizing = boxHeight (nodeConfig node)

propagateResolvedHeights :: LayoutNode measureM placed -> LayoutNode measureM placed
propagateResolvedHeights node =
  resize (node {nodeChildren = propagatedChildren})
  where
    propagatedChildren = propagateResolvedHeights <$> nodeChildren node
    config = nodeConfig node
    resize current
      | null (nodeChildren current) = current
      | boxDirection config == LeftToRight =
          current
            { nodeDimensions =
                (nodeDimensions current)
                  { sizeHeight =
                      clampNodeHeight current $
                        maximum
                          ( sizeHeight (nodeDimensions current)
                              : [ sizeHeight (nodeDimensions child)
                                    + insetTop (boxPadding config)
                                    + insetBottom (boxPadding config)
                                | child <- nodeChildren current
                                ]
                          )
                  }
            }
      | otherwise =
          current
            { nodeDimensions =
                (nodeDimensions current)
                  { sizeHeight =
                      clampNodeHeight current $
                        insetTop (boxPadding config)
                          + insetBottom (boxPadding config)
                          + sum (sizeHeight . nodeDimensions <$> nodeChildren current)
                          + gapSize (boxGap config) (nodeChildren current)
                  }
            }

wrapTextNodes :: LayoutNode measureM placed -> LayoutNode measureM placed
wrapTextNodes node =
  node
    { nodeText = wrappedText
    , nodeChildren = wrapTextNodes <$> nodeChildren node
    , nodeDimensions = wrappedDimensions
    }
  where
    wrappedText = wrapTextNode (sizeWidth (nodeDimensions node)) <$> nodeText node
    wrappedDimensions =
      case wrappedText of
        Nothing -> nodeDimensions node
        Just textNode ->
          (nodeDimensions node) {sizeHeight = textNodeLineHeight textNode * fromIntegral (length (textNodeLines textNode))}

scaleAspectHeights :: LayoutNode measureM placed -> LayoutNode measureM placed
scaleAspectHeights node =
  adjust node {nodeChildren = scaleAspectHeights <$> nodeChildren node}
  where
    adjust current =
      case nodeAspectRatio current of
        Just ratio
          | ratio /= 0 ->
              current
                { nodeDimensions = (nodeDimensions current) {sizeHeight = aspectHeight}
                , nodeHeightMaxOverride = Just aspectHeight
                }
          where
            aspectHeight = sizeWidth (nodeDimensions current) / ratio
        _ -> current

scaleAspectWidths :: LayoutNode measureM placed -> LayoutNode measureM placed
scaleAspectWidths node =
  adjust node {nodeChildren = scaleAspectWidths <$> nodeChildren node}
  where
    adjust current =
      case nodeAspectRatio current of
        Just ratio ->
          current {nodeDimensions = (nodeDimensions current) {sizeWidth = ratio * sizeHeight (nodeDimensions current)}}
        Nothing -> current

placeLayoutNode :: (Monad measureM, Monoid placed) => Point -> LayoutNode measureM placed -> measureM placed
placeLayoutNode point node = do
  own <- placeOwnNode point node
  children <- placeChildren point node
  pure (own <> children)

placeOwnNode :: (Monad measureM, Monoid placed) => Point -> LayoutNode measureM placed -> measureM placed
placeOwnNode point@Point {pointX, pointY} node = do
  placed <- foldM place mempty (reverse (nodePlacers node))
  textPlaced <- placeTextNode point node
  pure (placed <> textPlaced)
  where
    rect = Rect pointX pointY (sizeWidth (nodeDimensions node)) (sizeHeight (nodeDimensions node))
    place placed next = (placed <>) <$> next rect

placeTextNode :: (Monad measureM, Monoid placed) => Point -> LayoutNode measureM placed -> measureM placed
placeTextNode _ LayoutNode {nodeText = Nothing} =
  pure mempty
placeTextNode Point {pointX, pointY} LayoutNode {nodeText = Just textNode, nodeDimensions = Size {sizeWidth}} =
  snd <$> foldM placeLine (pointY, mempty) (zip [0 ..] (textNodeLines textNode))
  where
    lineHeight = textNodeLineHeight textNode
    TextConfig {textPlaceLine, textAlign} = textNodeConfig textNode
    placeLine (lineY, placed) (lineIndex, TextLine {lineText, lineWidth}) = do
      next <- textPlaceLine lineIndex lineText (Rect (pointX + alignOffset lineWidth) lineY lineWidth lineHeight)
      pure (lineY + lineHeight, placed <> next)
    alignOffset lineWidth =
      case textAlign of
        TextAlignStart -> 0
        TextAlignCenter -> (sizeWidth - lineWidth) / 2
        TextAlignEnd -> sizeWidth - lineWidth

placeChildren :: (Monad measureM, Monoid placed) => Point -> LayoutNode measureM placed -> measureM placed
placeChildren Point {pointX, pointY} node =
  snd <$> foldM placeChild (startingPrimary, mempty) (nodeChildren node)
  where
    config = nodeConfig node
    primaryAxis =
      case boxDirection config of
        LeftToRight -> Horizontal
        TopToBottom -> Vertical
    crossAxis = otherAxis primaryAxis
    boxInsets = boxPadding config
    inner =
      insetRect
        boxInsets
        (Rect pointX pointY (sizeWidth (nodeDimensions node)) (sizeHeight (nodeDimensions node)))
    childSizes = nodeDimensions <$> nodeChildren node
    contentPrimary = sum (axisSize primaryAxis <$> childSizes) + gapSize (boxGap config) childSizes
    extraPrimary = max 0 (axisSize primaryAxis (nodeDimensions node) - axisPadding primaryAxis boxInsets - contentPrimary)
    startingPrimary = rectAxisPosition primaryAxis inner + mainAlignmentOffset (boxMainAlign config) extraPrimary
    resolvedGap = mainAlignmentGap (boxMainAlign config) (boxGap config) extraPrimary (length (nodeChildren node))
    childOffset = nodeChildOffset node
    placeChild (primaryPosition, placed) child = do
      let childSize = nodeDimensions child
      let childPrimary = axisSize primaryAxis childSize
      let childCross = axisSize crossAxis childSize
      let crossPosition =
            rectAxisPosition crossAxis inner
              + crossAlignmentOffset (boxCrossAlign config) (axisSize crossAxis (shrinkSize boxInsets (nodeDimensions node))) childCross
      next <- placeLayoutNode (offsetPoint childOffset (pointFromAxes primaryAxis primaryPosition crossPosition)) child
      pure (primaryPosition + childPrimary + resolvedGap, placed <> next)

measureTextNode :: Monad measureM => TextConfig measureM placed -> String -> measureM (TextNode measureM placed)
measureTextNode config string = do
  spaceSize <- textMeasure config " "
  measurement <- measureTextTokens config string (sizeWidth spaceSize)
  pure
    TextNode
      { textNodeText = string
      , textNodeConfig = config
      , textNodePreferredSize = Size (measurementWidth measurement) (lineHeight measurement)
      , textNodeMinWidth = measurementMinWidth measurement
      , textNodeSpaceWidth = sizeWidth spaceSize
      , textNodeTokens = measurementTokens measurement
      , textNodeContainsNewlines = measurementContainsNewlines measurement
      , textNodeLines = [TextLine string (measurementWidth measurement)]
      }
  where
    lineHeight measurement =
      case textLineHeight config of
        Just value -> value
        Nothing -> measurementHeight measurement

data TextMeasurement = TextMeasurement
  { measurementWidth :: Double
  , measurementHeight :: Double
  , measurementMinWidth :: Double
  , measurementTokens :: [TextToken]
  , measurementContainsNewlines :: Bool
  }

measureTextTokens :: Monad measureM => TextConfig measureM placed -> String -> Double -> measureM TextMeasurement
measureTextTokens config string spaceWidth =
  finish =<< foldM step initialState string
  where
    initialState =
      TextMeasureState
        { pendingChars = ""
        , currentLineWidth = 0
        , widestLineWidth = 0
        , tallestWord = 0
        , widestWord = 0
        , tokensReversed = []
        , containsNewlines = False
        }
    step state character
      | character == ' ' = appendSpaceWord config spaceWidth state
      | character == '\n' = appendNewlineWord config state
      | otherwise = pure state {pendingChars = pendingChars state <> [character]}
    finish state = do
      finished <- appendPendingWord config state
      pure
        TextMeasurement
          { measurementWidth = max (currentLineWidth finished) (widestLineWidth finished)
          , measurementHeight = tallestWord finished
          , measurementMinWidth = widestWord finished
          , measurementTokens = reverse (tokensReversed finished)
          , measurementContainsNewlines = containsNewlines finished
          }

data TextMeasureState = TextMeasureState
  { pendingChars :: String
  , currentLineWidth :: Double
  , widestLineWidth :: Double
  , tallestWord :: Double
  , widestWord :: Double
  , tokensReversed :: [TextToken]
  , containsNewlines :: Bool
  }

appendSpaceWord :: Monad measureM => TextConfig measureM placed -> Double -> TextMeasureState -> measureM TextMeasureState
appendSpaceWord config spaceWidth state = do
  (word, wordSize) <- measurePendingWord config state
  let tokenText = word <> " "
  let tokenWidth = sizeWidth wordSize + spaceWidth
  pure
    state
      { pendingChars = ""
      , currentLineWidth = currentLineWidth state + tokenWidth
      , tallestWord = max (tallestWord state) (sizeHeight wordSize)
      , widestWord = max (widestWord state) (sizeWidth wordSize)
      , tokensReversed = TextWord tokenText tokenWidth : tokensReversed state
      }

appendNewlineWord :: Monad measureM => TextConfig measureM placed -> TextMeasureState -> measureM TextMeasureState
appendNewlineWord config state = do
  withWord <- appendPendingWord config state
  pure
    withWord
      { pendingChars = ""
      , currentLineWidth = 0
      , widestLineWidth = max (currentLineWidth withWord) (widestLineWidth withWord)
      , tokensReversed = TextNewline : tokensReversed withWord
      , containsNewlines = True
      }

appendPendingWord :: Monad measureM => TextConfig measureM placed -> TextMeasureState -> measureM TextMeasureState
appendPendingWord config state
  | null (pendingChars state) = pure state
  | otherwise = do
      (word, wordSize) <- measurePendingWord config state
      pure
        state
          { pendingChars = ""
          , currentLineWidth = currentLineWidth state + sizeWidth wordSize
          , tallestWord = max (tallestWord state) (sizeHeight wordSize)
          , widestWord = max (widestWord state) (sizeWidth wordSize)
          , tokensReversed = TextWord word (sizeWidth wordSize) : tokensReversed state
          }

measurePendingWord :: Monad measureM => TextConfig measureM placed -> TextMeasureState -> measureM (String, Size)
measurePendingWord TextConfig {textMeasure} TextMeasureState {pendingChars}
  | null pendingChars = pure ("", Size 0 0)
  | otherwise = (pendingChars,) <$> textMeasure pendingChars

wrapTextNode :: Double -> TextNode measureM placed -> TextNode measureM placed
wrapTextNode availableWidth textNode =
  textNode {textNodeLines = linesForMode}
  where
    linesForMode =
      case textWrapMode (textNodeConfig textNode) of
        TextWrapWords ->
          if not (textNodeContainsNewlines textNode) && sizeWidth (textNodePreferredSize textNode) <= availableWidth
            then [TextLine (textNodeText textNode) availableWidth]
            else wrapWords availableWidth (textNodeSpaceWidth textNode) (textNodeTokens textNode)
        TextWrapNewlines ->
          wrapNewlines (textNodeTokens textNode)
        TextWrapNone ->
          [TextLine (textNodeText textNode) (sizeWidth (textNodePreferredSize textNode))]

wrapWords :: Double -> Double -> [TextToken] -> [TextLine]
wrapWords availableWidth spaceWidth =
  finish . foldl step ([], "", 0)
  where
    finish (linesReversed, currentText, currentWidth)
      | null currentText = reverse linesReversed
      | otherwise = reverse (trimmedLine currentText currentWidth : linesReversed)
    step (linesReversed, currentText, currentWidth) token =
      case token of
        TextNewline ->
          (trimmedLine currentText currentWidth : linesReversed, "", 0)
        TextWord word wordWidth
          | null currentText && wordWidth > availableWidth ->
              (TextLine word wordWidth : linesReversed, "", 0)
          | not (null currentText) && currentWidth + wordWidth > availableWidth && wordWidth > availableWidth ->
              (TextLine word wordWidth : trimmedLine currentText currentWidth : linesReversed, "", 0)
          | not (null currentText) && currentWidth + wordWidth > availableWidth ->
              (trimmedLine currentText currentWidth : linesReversed, word, wordWidth)
          | otherwise ->
              (linesReversed, currentText <> word, currentWidth + wordWidth)
    trimmedLine currentText currentWidth =
      if endsWithSpace currentText
        then TextLine (init currentText) (currentWidth - spaceWidth)
        else TextLine currentText currentWidth

wrapNewlines :: [TextToken] -> [TextLine]
wrapNewlines =
  finish . foldl step ([], "", 0)
  where
    finish (linesReversed, currentText, currentWidth) =
      reverse (TextLine currentText currentWidth : linesReversed)
    step (linesReversed, currentText, currentWidth) token =
      case token of
        TextNewline -> (TextLine currentText currentWidth : linesReversed, "", 0)
        TextWord word wordWidth -> (linesReversed, currentText <> word, currentWidth + wordWidth)

textNodeLineHeight :: TextNode measureM placed -> Double
textNodeLineHeight textNode =
  sizeHeight (textNodePreferredSize textNode)

endsWithSpace :: String -> Bool
endsWithSpace [] = False
endsWithSpace string = last string == ' '

data Axis
  = Horizontal
  | Vertical
  deriving (Eq)

otherAxis :: Axis -> Axis
otherAxis Horizontal = Vertical
otherAxis Vertical = Horizontal

axisSize :: Axis -> Size -> Double
axisSize Horizontal = sizeWidth
axisSize Vertical = sizeHeight

axisSizing :: Axis -> Sizing -> AxisSizing
axisSizing Horizontal = sizingWidth
axisSizing Vertical = sizingHeight

updateNodeAxisDimension :: Axis -> Double -> LayoutNode measureM placed -> LayoutNode measureM placed
updateNodeAxisDimension Horizontal value node =
  node {nodeDimensions = (nodeDimensions node) {sizeWidth = value}}
updateNodeAxisDimension Vertical value node =
  node {nodeDimensions = (nodeDimensions node) {sizeHeight = value}}

axisIsAlongDirection :: Axis -> Direction -> Bool
axisIsAlongDirection Horizontal LeftToRight = True
axisIsAlongDirection Vertical TopToBottom = True
axisIsAlongDirection _ _ = False

nodeClipsAxis :: Axis -> LayoutNode measureM placed -> Bool
nodeClipsAxis Horizontal node =
  clipHorizontal (boxClip (nodeConfig node))
nodeClipsAxis Vertical node =
  clipVertical (boxClip (nodeConfig node))

nodeChildOffset :: LayoutNode measureM placed -> Point
nodeChildOffset node =
  if clipHorizontal clipConfig || clipVertical clipConfig
    then clipChildOffset clipConfig
    else Point 0 0
  where
    clipConfig = boxClip (nodeConfig node)

offsetPoint :: Point -> Point -> Point
offsetPoint Point {pointX = offsetX, pointY = offsetY} Point {pointX, pointY} =
  Point (pointX + offsetX) (pointY + offsetY)

axisPadding :: Axis -> Insets -> Double
axisPadding Horizontal Insets {insetLeft, insetRight} = insetLeft + insetRight
axisPadding Vertical Insets {insetTop, insetBottom} = insetTop + insetBottom

shrinkSize :: Insets -> Size -> Size
shrinkSize Insets {insetTop, insetRight, insetBottom, insetLeft} Size {sizeWidth, sizeHeight} =
  Size
    { sizeWidth = sizeWidth - insetLeft - insetRight
    , sizeHeight = sizeHeight - insetTop - insetBottom
    }

closeAxisSize :: AxisSizing -> Double -> Double
closeAxisSize sizing value =
  case stripClamp sizing of
    Percent {} -> 0
    _ -> clampAxis sizing value

closeAxisMinSize :: AxisSizing -> Double -> Double
closeAxisMinSize sizing value =
  case stripClamp sizing of
    Percent {} -> value
    _ -> clampAxis sizing value

distributeGrowNodes :: Axis -> Double -> [LayoutNode measureM placed] -> [Int] -> [LayoutNode measureM placed]
distributeGrowNodes axis remaining nodes activeIndices
  | remaining <= clayEpsilon = nodes
  | null activeIndices = nodes
  | otherwise = distributeGrowNodes axis remainingAfterAdd nodesAfterAdd activeAfterAdd
  where
    sizes = axisSize axis . nodeDimensions <$> nodes
    (smallest, growAmount) = growStep remaining sizes activeIndices
    (remainingAfterAdd, nodesAfterAdd, activeAfterAdd) =
      foldr addGrowStep (remaining, nodes, []) activeIndices
    addGrowStep index (remainingSoFar, nodesSoFar, activeSoFar)
      | not (clayFloatEqual (nodeAxisSize index nodesSoFar) smallest) =
          (remainingSoFar, nodesSoFar, index : activeSoFar)
      | otherwise =
          let previousSize = nodeAxisSize index nodesSoFar
              maxSize = nodeAxisMax axis (nodesSoFar !! index)
              grown = previousSize + growAmount
              newSize = min grown maxSize
              newNodes = replaceAt index (updateNodeAxisDimension axis newSize (nodesSoFar !! index)) nodesSoFar
              newRemaining = remainingSoFar - (newSize - previousSize)
              newActive =
                if newSize >= maxSize
                  then activeSoFar
                  else index : activeSoFar
           in (newRemaining, newNodes, newActive)
    nodeAxisSize index = axisSize axis . nodeDimensions . (!! index)

growStep :: Double -> [Double] -> [Int] -> (Double, Double)
growStep remaining sizes activeIndices =
  (smallest, min widthToAdd (remaining / fromIntegral (length activeIndices)))
  where
    (smallest, _secondSmallest, widthToAdd) =
      foldl inspect (clayMaxFloat, clayMaxFloat, remaining) activeIndices
    inspect (smallestSoFar, secondSmallestSoFar, widthToAddSoFar) index
      | clayFloatEqual childSize smallestSoFar =
          (smallestSoFar, secondSmallestSoFar, widthToAddSoFar)
      | childSize < smallestSoFar =
          (childSize, smallestSoFar, widthToAddSoFar)
      | childSize > smallestSoFar =
          let secondSmallest = min secondSmallestSoFar childSize
           in (smallestSoFar, secondSmallest, secondSmallest - smallestSoFar)
      | otherwise =
          (smallestSoFar, secondSmallestSoFar, widthToAddSoFar)
      where
        childSize = sizes !! index

distributeCompressNodes :: Axis -> Double -> [LayoutNode measureM placed] -> [Int] -> [LayoutNode measureM placed]
distributeCompressNodes axis remaining nodes activeIndices
  | remaining >= negate clayEpsilon = nodes
  | null activeIndices = nodes
  | otherwise = distributeCompressNodes axis remainingAfterAdd nodesAfterAdd activeAfterAdd
  where
    sizes = axisSize axis . nodeDimensions <$> nodes
    (largest, shrinkAmount) = compressStep remaining sizes activeIndices
    (remainingAfterAdd, nodesAfterAdd, activeAfterAdd) =
      foldr addCompressStep (remaining, nodes, []) activeIndices
    addCompressStep index (remainingSoFar, nodesSoFar, activeSoFar)
      | not (clayFloatEqual (nodeAxisSize index nodesSoFar) largest) =
          (remainingSoFar, nodesSoFar, index : activeSoFar)
      | otherwise =
          let previousSize = nodeAxisSize index nodesSoFar
              minSize = axisSize axis (nodeMinDimensions (nodesSoFar !! index))
              shrunk = previousSize + shrinkAmount
              newSize = max shrunk minSize
              newNodes = replaceAt index (updateNodeAxisDimension axis newSize (nodesSoFar !! index)) nodesSoFar
              newRemaining = remainingSoFar - (newSize - previousSize)
              newActive =
                if newSize <= minSize
                  then activeSoFar
                  else index : activeSoFar
           in (newRemaining, newNodes, newActive)
    nodeAxisSize index = axisSize axis . nodeDimensions . (!! index)

compressStep :: Double -> [Double] -> [Int] -> (Double, Double)
compressStep remaining sizes activeIndices =
  (largest, max widthToAdd (remaining / fromIntegral (length activeIndices)))
  where
    (largest, _secondLargest, widthToAdd) =
      foldl inspect (0, 0, remaining) activeIndices
    inspect (largestSoFar, secondLargestSoFar, widthToAddSoFar) index
      | clayFloatEqual childSize largestSoFar =
          (largestSoFar, secondLargestSoFar, widthToAddSoFar)
      | childSize > largestSoFar =
          (childSize, largestSoFar, widthToAddSoFar)
      | childSize < largestSoFar =
          let secondLargest = max secondLargestSoFar childSize
           in (largestSoFar, secondLargest, secondLargest - largestSoFar)
      | otherwise =
          (largestSoFar, secondLargestSoFar, widthToAddSoFar)
      where
        childSize = sizes !! index

isFill :: AxisSizing -> Bool
isFill sizing =
  case stripClamp sizing of
    Fill -> True
    _ -> False

isFixed :: AxisSizing -> Bool
isFixed sizing =
  case stripClamp sizing of
    Fixed {} -> True
    _ -> False

isPercent :: AxisSizing -> Bool
isPercent sizing =
  case stripClamp sizing of
    Percent {} -> True
    _ -> False

isResizable :: AxisSizing -> Bool
isResizable sizing =
  not (isFixed sizing || isPercent sizing)

nodeCanResizeAlongAxis :: Axis -> LayoutNode measureM placed -> Bool
nodeCanResizeAlongAxis axis node =
  isResizable (axisSizing axis (nodeSizing node))
    && textNodeCanResize node

textNodeCanResize :: LayoutNode measureM placed -> Bool
textNodeCanResize LayoutNode {nodeText = Nothing} = True
textNodeCanResize LayoutNode {nodeText = Just textNode} =
  textWrapMode (textNodeConfig textNode) == TextWrapWords

stripClamp :: AxisSizing -> AxisSizing
stripClamp (Clamp _ _ sizing) = stripClamp sizing
stripClamp sizing = sizing

clampAxis :: AxisSizing -> Double -> Double
clampAxis sizing value =
  case sizing of
    Fixed fixedValue -> fixedValue
    Clamp maybeMin maybeMax inner ->
      clampMax maybeMax (clampMin maybeMin (clampAxis inner value))
    _ -> value
  where
    clampMin Nothing = id
    clampMin (Just minimumValue) = max minimumValue
    clampMax Nothing = id
    clampMax (Just maximumValue) = min maximumValue

axisMax :: AxisSizing -> Double
axisMax (Clamp _ maybeMax sizing) =
  case maybeMax of
    Just maximumValue -> min maximumValue (axisMax sizing)
    Nothing -> axisMax sizing
axisMax (Fixed value) = value
axisMax _ = clayMaxFloat

axisMin :: AxisSizing -> Double
axisMin (Clamp maybeMin _ sizing) =
  case maybeMin of
    Just minimumValue -> max minimumValue (axisMin sizing)
    Nothing -> axisMin sizing
axisMin (Fixed value) = value
axisMin _ = 0

nodeAxisMax :: Axis -> LayoutNode measureM placed -> Double
nodeAxisMax Horizontal node =
  axisMax (boxWidth (nodeConfig node))
nodeAxisMax Vertical node =
  case (isPercent sizing, nodeHeightMaxOverride node) of
    (True, Just maximumValue) -> maximumValue
    (True, Nothing) -> 0
    (_, Just maximumValue) -> maximumValue
    (_, Nothing) -> axisMax sizing
  where
    sizing = boxHeight (nodeConfig node)

percentValue :: AxisSizing -> Maybe Double
percentValue (Clamp _ _ sizing) = percentValue sizing
percentValue (Percent value) = Just value
percentValue _ = Nothing

replaceAt :: Int -> item -> [item] -> [item]
replaceAt index value items =
  take index items <> [value] <> drop (index + 1) items

clayEpsilon :: Double
clayEpsilon = 0.01

clayMaxFloat :: Double
clayMaxFloat = 3.4028234663852886e38

clayFloatEqual :: Double -> Double -> Bool
clayFloatEqual left right =
  difference < clayEpsilon && difference > -clayEpsilon
  where
    difference = left - right

fillMissingAspectDimension :: Double -> Size -> Size
fillMissingAspectDimension ratio size@Size {sizeWidth, sizeHeight}
  | ratio == 0 = size
  | dimensionIsMissing sizeWidth && not (dimensionIsMissing sizeHeight) = Size (sizeHeight * ratio) sizeHeight
  | not (dimensionIsMissing sizeWidth) && dimensionIsMissing sizeHeight = Size sizeWidth (sizeWidth / ratio)
  | otherwise = size

dimensionIsMissing :: Double -> Bool
dimensionIsMissing value =
  abs value < numericZeroEpsilon

numericZeroEpsilon :: Double
numericZeroEpsilon = 1e-9

mainAlignmentOffset :: MainAlign -> Double -> Double
mainAlignmentOffset MainStart _extra = 0
mainAlignmentOffset MainCenter extra = extra / 2
mainAlignmentOffset MainEnd extra = extra

mainAlignmentGap :: MainAlign -> Double -> Double -> Int -> Double
mainAlignmentGap _ gap _extra _childCount = gap

crossAlignmentOffset :: CrossAlign -> Double -> Double -> Double
crossAlignmentOffset CrossStart _available _child = 0
crossAlignmentOffset CrossCenter available child = (available - child) / 2
crossAlignmentOffset CrossEnd available child = available - child

gapSize :: Double -> [item] -> Double
gapSize gap items =
  gap * fromIntegral (max 0 (length items - 1))

maximumOrZero :: [Double] -> Double
maximumOrZero [] = 0
maximumOrZero values = maximum values

rectAxisPosition :: Axis -> Rect -> Double
rectAxisPosition Horizontal = x
rectAxisPosition Vertical = y

pointFromAxes :: Axis -> Double -> Double -> Point
pointFromAxes Horizontal primaryPosition crossPosition =
  Point primaryPosition crossPosition
pointFromAxes Vertical primaryPosition crossPosition =
  Point crossPosition primaryPosition
