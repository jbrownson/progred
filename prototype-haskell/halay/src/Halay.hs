module Halay
  ( AxisSizing (..)
  , BoxConfig (..)
  , BoxClip (..)
  , CrossAlign (..)
  , Direction (..)
  , Halay
  , MainAlign (..)
  , Measured (..)
  , MinMax (..)
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
  , unbounded
  , module Halay.Geometry
  ) where

import Control.Monad (foldM)
import Data.List (mapAccumL)
import Halay.Geometry

data AxisSizing
  = Fit MinMax
  | Fixed Double
  | Fill MinMax
  | Percent Double
  deriving (Eq, Show)

-- Mirrors Clay's Clay_SizingMinMax: only fit and grow sizing carry min/max
-- bounds; fixed is exactly min == max == value.
data MinMax = MinMax (Maybe Double) (Maybe Double)
  deriving (Eq, Show)

unbounded :: MinMax
unbounded = MinMax Nothing Nothing

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
  , boxSizing :: Sizing
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
  , placeMeasured :: Rect -> measureM placed
  }

data LayoutNode measureM placed = LayoutNode
  { nodeConfig :: BoxConfig
  , nodeAspectRatio :: Maybe Double
  , nodeHeightMaxOverride :: Maybe Double
  , nodeContent :: NodeContent measureM placed
  , nodePlacers :: [Rect -> measureM placed]
  , nodeChildren :: [LayoutNode measureM placed]
  , nodeDimensions :: Size
  , nodeMinDimensions :: Size
  }

data NodeContent measureM placed
  = Container
  | Intrinsic Size
  | TextContent (TextNode measureM placed)

data TextNode measureM placed = TextNode
  { textNodeText :: String
  , textNodeConfig :: TextConfig measureM placed
  , textNodePreferredSize :: Size
  , textNodeNaturalLineHeight :: Double
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
defaultSizing = Sizing (Fit unbounded) (Fit unbounded)

defaultBox :: BoxConfig
defaultBox =
  BoxConfig
    { boxDirection = LeftToRight
    , boxPadding = Insets 0 0 0 0
    , boxGap = 0
    , boxSizing = defaultSizing
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
        { nodeConfig = defaultBox {boxSizing = sizing}
        , nodeContent = Intrinsic intrinsicSize
        , nodePlacers = [place]
        }

text :: Monad measureM => TextConfig measureM placed -> String -> Halay measureM placed
text config string =
  Halay $ do
    measured <- measureTextNode config string
    pure
      emptyNode
        { nodeContent = TextContent measured
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

placeHalay :: (Monad measureM, Monoid placed) => Rect -> Halay measureM placed -> measureM placed
placeHalay rect halay = do
  measured <- measureHalay halay
  placeMeasured measured rect

placeAt :: (Monad measureM, Monoid placed) => Point -> Halay measureM placed -> measureM (Size, placed)
placeAt point halay = do
  measured <- measureHalay halay
  placed <- placeMeasured measured (sizeRectAt point (measuredSize measured))
  pure (measuredSize measured, placed)

measureHalay :: (Monad measureM, Monoid placed) => Halay measureM placed -> measureM (Measured measureM placed)
measureHalay halay = do
  source <- buildHalay halay
  let measuredNode = layoutNode Nothing source
  pure
    Measured
      { measuredSize = nodeDimensions measuredNode
      , placeMeasured = \rect ->
          placeLayoutNode (Point (x rect) (y rect)) (layoutNode (Just (Size (width rect) (height rect))) source)
      }

row :: Applicative measureM => [Halay measureM placed] -> Halay measureM placed
row =
  rowWithGap 0

rowWithGap :: Applicative measureM => Double -> [Halay measureM placed] -> Halay measureM placed
rowWithGap gap =
  box defaultBox {boxDirection = LeftToRight, boxGap = gap}

column :: Applicative measureM => [Halay measureM placed] -> Halay measureM placed
column =
  columnWithGap 0

columnWithGap :: Applicative measureM => Double -> [Halay measureM placed] -> Halay measureM placed
columnWithGap gap =
  box defaultBox {boxDirection = TopToBottom, boxGap = gap}

padding :: Applicative measureM => Insets -> Halay measureM placed -> Halay measureM placed
padding insets child =
  box defaultBox {boxPadding = insets} [child]

box :: Applicative measureM => BoxConfig -> [Halay measureM placed] -> Halay measureM placed
box config children =
  Halay $
    makeBox <$> traverse buildHalay children
  where
    makeBox childNodes =
      emptyNode {nodeConfig = config, nodeChildren = childNodes}

emptyNode :: LayoutNode measureM placed
emptyNode =
  LayoutNode
    { nodeConfig = defaultBox
    , nodeAspectRatio = Nothing
    , nodeHeightMaxOverride = Nothing
    , nodeContent = Container
    , nodePlacers = []
    , nodeChildren = []
    , nodeDimensions = Size 0 0
    , nodeMinDimensions = Size 0 0
    }

setNodeSizing :: Sizing -> LayoutNode measureM placed -> LayoutNode measureM placed
setNodeSizing sizing node =
  node {nodeConfig = (nodeConfig node) {boxSizing = sizing}}

setNodeAspectRatio :: Double -> LayoutNode measureM placed -> LayoutNode measureM placed
setNodeAspectRatio ratio node =
  node {nodeAspectRatio = Just ratio}

addNodePlacer :: (Rect -> measureM placed) -> LayoutNode measureM placed -> LayoutNode measureM placed
addNodePlacer place node =
  node {nodePlacers = place : nodePlacers node}

nodeSizing :: LayoutNode measureM placed -> Sizing
nodeSizing LayoutNode {nodeConfig = BoxConfig {boxSizing}} =
  boxSizing

layoutNode :: Maybe Size -> LayoutNode measureM placed -> LayoutNode measureM placed
layoutNode rootSizeOverride =
  layoutTree . overrideRootSize rootSizeOverride . closeNode

layoutTree :: LayoutNode measureM placed -> LayoutNode measureM placed
layoutTree root =
  mapNodes scaleAspectWidth afterVerticalSizing
  where
    afterHorizontalSizing = sizeAlongAxis Horizontal root
    wrappedText = mapNodes wrapNodeText afterHorizontalSizing
    aspectHeights = mapNodes scaleAspectHeight wrappedText
    propagatedHeights = propagateResolvedHeights aspectHeights
    afterVerticalSizing = sizeAlongAxis Vertical propagatedHeights

mapNodes :: (LayoutNode measureM placed -> LayoutNode measureM placed) -> LayoutNode measureM placed -> LayoutNode measureM placed
mapNodes change node =
  change node {nodeChildren = mapNodes change <$> nodeChildren node}

sizeAlongAxis :: Axis -> LayoutNode measureM placed -> LayoutNode measureM placed
sizeAlongAxis axis parent =
  parent {nodeChildren = sizeAlongAxis axis <$> sizeChildrenAlongAxis axis parent}

data ParentScan = ParentScan
  { scanInnerContentSize :: Double
  , scanTotalPaddingAndChildGaps :: Double
  , scanGrowContainerCount :: Int
  , scanResizableChildren :: [Int]
  }

sizeChildrenAlongAxis :: Axis -> LayoutNode measureM placed -> [LayoutNode measureM placed]
sizeChildrenAlongAxis axis parent =
  distributeParentSpace percentChildren percentInnerContentSize
  where
    children = nodeChildren parent
    config = nodeConfig parent
    parentSize = axisSize axis (nodeDimensions parent)
    parentPadding = axisPadding axis (boxPadding config)
    sizingAlongAxis = axis == directionAxis (boxDirection config)
    initialScan =
      ParentScan
        { scanInnerContentSize = 0
        , scanTotalPaddingAndChildGaps = parentPadding
        , scanGrowContainerCount = 0
        , scanResizableChildren = []
        }
    scan = foldl scanChild initialScan (zip [0 ..] children)
    scanChild current (childIndex, child) =
      current
        { scanInnerContentSize = nextInnerContentSize
        , scanTotalPaddingAndChildGaps = nextTotalPaddingAndChildGaps
        , scanGrowContainerCount = nextGrowContainerCount
        , scanResizableChildren = scanResizableChildren current <> resizableChild
        }
      where
        childSizing = nodeAxisSizing axis child
        childSize = axisSize axis (nodeDimensions child)
        childGap =
          if childIndex == 0 || not sizingAlongAxis
            then 0
            else boxGap config
        nextInnerContentSize =
          if sizingAlongAxis
            then
              clayAdd
                (clayAdd (scanInnerContentSize current) childGap)
                (if isPercent childSizing then 0 else childSize)
            else max childSize (scanInnerContentSize current)
        nextTotalPaddingAndChildGaps =
          clayAdd (scanTotalPaddingAndChildGaps current) childGap
        nextGrowContainerCount =
          scanGrowContainerCount current
            + if isFill childSizing then 1 else 0
        resizableChild =
          [ childIndex
          | not (isPercent childSizing)
          , not (isFixed childSizing)
          , textNodeCanResize child
          ]
    (percentInnerContentSize, percentChildren) =
      mapAccumL expandPercentChild (scanInnerContentSize scan) children
    expandPercentChild innerContentSize child =
      case nodeAxisSizing axis child of
        Percent percent ->
          ( if sizingAlongAxis then clayAdd innerContentSize percentSize else innerContentSize
          , updateMissingAspectDimension (updateNodeAxisDimension axis percentSize child)
          )
          where
            percentSize = clayMul (claySub parentSize (scanTotalPaddingAndChildGaps scan)) percent
        _ -> (innerContentSize, child)
    distributeParentSpace currentChildren innerContentSize
      | sizingAlongAxis && sizeToDistribute < 0 && nodeClipsAxis axis parent =
          currentChildren
      | sizingAlongAxis && sizeToDistribute < 0 =
          distributeChildren CompressNodes axis sizeToDistribute currentChildren (scanResizableChildren scan)
      | sizingAlongAxis && sizeToDistribute > 0 && scanGrowContainerCount scan > 0 =
          distributeChildren GrowNodes axis sizeToDistribute currentChildren growChildren
      | sizingAlongAxis =
          currentChildren
      | otherwise =
          resolveCrossAxisChildren axis parent parentSize parentPadding innerContentSize currentChildren (scanResizableChildren scan)
      where
        sizeToDistribute = claySub (claySub parentSize parentPadding) innerContentSize
        growChildren =
          filter
            (\childIndex -> isFill (nodeAxisSizing axis (currentChildren !! childIndex)))
            (scanResizableChildren scan)

nodeAxisSizing :: Axis -> LayoutNode measureM placed -> AxisSizing
nodeAxisSizing axis =
  axisSizing axis . nodeSizing

nodeIsText :: LayoutNode measureM placed -> Bool
nodeIsText LayoutNode {nodeContent = TextContent _} = True
nodeIsText _ = False

resolveCrossAxisChildren :: Axis -> LayoutNode measureM placed -> Double -> Double -> Double -> [LayoutNode measureM placed] -> [Int] -> [LayoutNode measureM placed]
resolveCrossAxisChildren axis parent parentSize parentPadding innerContentSize =
  foldl resolve
  where
    maxSize
      | nodeClipsAxis axis parent = max visibleMaxSize innerContentSize
      | otherwise = visibleMaxSize
    visibleMaxSize = claySub parentSize parentPadding
    resolve currentChildren childIndex =
      modifyAt childIndex resize currentChildren
      where
        child = currentChildren !! childIndex
        childSizing = nodeAxisSizing axis child
        minSize = axisSize axis (nodeMinDimensions child)
        resize childNode =
          updateNodeAxisDimension axis resolvedSize childNode
        resolvedSize =
          max minSize $
            min maxSize $
              if isFill childSizing
                then min maxSize (nodeAxisMax axis child)
                else axisSize axis (nodeDimensions child)

distributeChildren :: DistributionMode -> Axis -> Double -> [LayoutNode measureM placed] -> [Int] -> [LayoutNode measureM placed]
distributeChildren mode axis remaining children activeIndices
  | distributionComplete mode remaining = children
  | null activeIndices = children
  | otherwise =
      distributeChildren mode axis remainingAfterPass childrenAfterPass activeAfterPass
  where
    (frontierSize, resizeAmount) = distributionStep mode axis remaining children activeIndices
    (remainingAfterPass, childrenAfterPass, activeAfterPass) =
      applyDistributionPass mode axis frontierSize resizeAmount remaining children activeIndices

-- Mirrors Clay's in-order pass with swap-back removal; the iteration order
-- affects float accumulation and which nodes reach their bounds first.
applyDistributionPass :: DistributionMode -> Axis -> Double -> Double -> Double -> [LayoutNode measureM placed] -> [Int] -> (Double, [LayoutNode measureM placed], [Int])
applyDistributionPass mode axis frontierSize resizeAmount =
  step 0
  where
    step position remaining children activeIndices
      | position >= length activeIndices = (remaining, children, activeIndices)
      | not (clayFloatEqual previousSize frontierSize) =
          step (position + 1) remaining children activeIndices
      | otherwise =
          step nextPosition nextRemaining nextChildren nextActiveIndices
      where
        childIndex = activeIndices !! position
        child = children !! childIndex
        previousSize = axisSize axis (nodeDimensions child)
        bound = distributionBound mode axis child
        newSize = applyDistribution mode previousSize resizeAmount bound
        nextRemaining = claySub remaining (claySub newSize previousSize)
        nextChildren = modifyAt childIndex (updateNodeAxisDimension axis newSize) children
        atBound = distributionAtBound mode newSize bound
        nextActiveIndices =
          if atBound
            then removeSwapbackAt position activeIndices
            else activeIndices
        nextPosition =
          if atBound
            then position
            else position + 1

distributionStep :: DistributionMode -> Axis -> Double -> [LayoutNode measureM placed] -> [Int] -> (Double, Double)
distributionStep GrowNodes axis remaining children activeIndices =
  (smallest, min widthToAdd (clayDiv remaining (fromIntegral (length activeIndices))))
  where
    (smallest, _secondSmallest, widthToAdd) =
      foldl inspect (clayMaxFloat, clayMaxFloat, remaining) activeIndices
    inspect (smallestSoFar, secondSmallestSoFar, widthToAddSoFar) childIndex
      | clayFloatEqual childSize smallestSoFar =
          (smallestSoFar, secondSmallestSoFar, widthToAddSoFar)
      | childSize < smallestSoFar =
          (childSize, smallestSoFar, widthToAddSoFar)
      | childSize > smallestSoFar =
          let secondSmallest = min secondSmallestSoFar childSize
           in (smallestSoFar, secondSmallest, claySub secondSmallest smallestSoFar)
      | otherwise =
          (smallestSoFar, secondSmallestSoFar, widthToAddSoFar)
      where
        childSize = axisSize axis (nodeDimensions (children !! childIndex))
distributionStep CompressNodes axis remaining children activeIndices =
  (largest, max widthToAdd (clayDiv remaining (fromIntegral (length activeIndices))))
  where
    (largest, _secondLargest, widthToAdd) =
      foldl inspect (0, 0, remaining) activeIndices
    inspect (largestSoFar, secondLargestSoFar, widthToAddSoFar) childIndex
      | clayFloatEqual childSize largestSoFar =
          (largestSoFar, secondLargestSoFar, widthToAddSoFar)
      | childSize > largestSoFar =
          (childSize, largestSoFar, widthToAddSoFar)
      | childSize < largestSoFar =
          let secondLargest = max secondLargestSoFar childSize
           in (largestSoFar, secondLargest, claySub secondLargest largestSoFar)
      | otherwise =
          (largestSoFar, secondLargestSoFar, widthToAddSoFar)
      where
        childSize = axisSize axis (nodeDimensions (children !! childIndex))

wrapNodeText :: LayoutNode measureM placed -> LayoutNode measureM placed
wrapNodeText node =
  case nodeContent node of
    TextContent textNode ->
      node
        { nodeContent = TextContent wrappedTextNode
        , nodeDimensions =
            (nodeDimensions node)
              { sizeHeight = clayMul (textNodeLineHeight wrappedTextNode) (fromIntegral (length (textNodeLines wrappedTextNode)))
              }
        }
      where
        wrappedTextNode = wrapTextNode (sizeWidth (nodeDimensions node)) textNode
    _ -> node

scaleAspectHeight :: LayoutNode measureM placed -> LayoutNode measureM placed
scaleAspectHeight node =
  case nodeAspectRatio node of
    Just ratio
      | not (nodeIsText node)
      , ratio /= 0 ->
          node
            { nodeDimensions = setSizeAxis Vertical aspectHeight (nodeDimensions node)
            , nodeHeightMaxOverride = Just aspectHeight
            }
      where
        -- Clay computes (1 / ratio) * width; the float32 rounding differs
        -- from width / ratio and can flip epsilon-level layout decisions.
        aspectHeight = clayMul (clayDiv 1 ratio) (sizeWidth (nodeDimensions node))
    _ -> node

scaleAspectWidth :: LayoutNode measureM placed -> LayoutNode measureM placed
scaleAspectWidth node =
  case nodeAspectRatio node of
    Just ratio
      | not (nodeIsText node)
      , ratio /= 0 ->
          updateNodeAxisDimension Horizontal (clayMul ratio (sizeHeight (nodeDimensions node))) node
    _ -> node

propagateResolvedHeights :: LayoutNode measureM placed -> LayoutNode measureM placed
propagateResolvedHeights node
  | nodeIsText node || null (nodeChildren node) = node
  | boxDirection config == LeftToRight = resizeRow withPropagatedChildren
  | otherwise = resizeColumn withPropagatedChildren
  where
    config = nodeConfig node
    withPropagatedChildren =
      node {nodeChildren = propagateResolvedHeights <$> nodeChildren node}
    resizeRow currentNode =
      foldl resizeForChild currentNode (nodeChildren currentNode)
    resizeForChild currentNode child =
      currentNode
        { nodeDimensions =
            (nodeDimensions currentNode)
              { sizeHeight =
                  normalizeClayDimension $
                    clampNodeHeight currentNode $
                      max
                        (sizeHeight (nodeDimensions currentNode))
                        ( clayAdd
                            (clayAdd (sizeHeight (nodeDimensions child)) (insetTop (boxPadding config)))
                            (insetBottom (boxPadding config))
                        )
              }
        }
    resizeColumn currentNode =
      currentNode
        { nodeDimensions =
            (nodeDimensions currentNode)
              { sizeHeight =
                  normalizeClayDimension $
                    clampNodeHeight currentNode $
                      clayAdd
                        ( clayAdd
                            (clayAdd (insetTop (boxPadding config)) (insetBottom (boxPadding config)))
                            (claySum [sizeHeight (nodeDimensions child) | child <- nodeChildren currentNode])
                        )
                        (gapSize (boxGap config) (nodeChildren currentNode))
              }
        }

overrideRootSize :: Maybe Size -> LayoutNode measureM placed -> LayoutNode measureM placed
overrideRootSize Nothing node = node
overrideRootSize (Just size) node = node {nodeDimensions = size}

closeNode :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNode node =
  updateMissingAspectDimension (closeNodeSizing closed)
  where
    closedChildren = closeNode <$> nodeChildren node
    withClosedChildren = node {nodeChildren = closedChildren}
    closed =
      case nodeContent node of
        TextContent textNode ->
          withClosedChildren
            { nodeDimensions = textNodePreferredSize textNode
            , nodeMinDimensions =
                Size
                  { sizeWidth = textNodeMinWidth textNode
                  , sizeHeight = textNodeLineHeight textNode
                  }
            }
        Intrinsic intrinsicSize ->
          withClosedChildren
            { nodeDimensions = intrinsicSize
            , nodeMinDimensions = intrinsicSize
            }
        Container ->
          closeContainer withClosedChildren

closeContainer :: LayoutNode measureM placed -> LayoutNode measureM placed
closeContainer node =
  node
    { nodeDimensions = clayExpandSize boxInsets contentSize
    , nodeMinDimensions = closeContainerMinDimensions config minContentSize
    }
  where
    config = nodeConfig node
    children = nodeChildren node
    boxInsets = boxPadding config
    childSizes = nodeDimensions <$> children
    childMinSizes = nodeMinDimensions <$> children
    contentSize = containerContentSize (boxDirection config) (boxGap config) childSizes
    minContentSize = containerContentSize (boxDirection config) (boxGap config) childMinSizes

closeContainerMinDimensions :: BoxConfig -> Size -> Size
closeContainerMinDimensions config contentSize =
  sizeFromAxes primaryAxis minPrimary minCross
  where
    primaryAxis = directionAxis (boxDirection config)
    crossAxis = otherAxis primaryAxis
    boxInsets = boxPadding config
    minPrimary =
      if boxClipsAxis primaryAxis config
        then axisPadding primaryAxis boxInsets
        else clayAdd (axisSize primaryAxis contentSize) (axisPadding primaryAxis boxInsets)
    minCross =
      if boxClipsAxis crossAxis config
        then 0
        else clayAdd (axisSize crossAxis contentSize) (axisPadding crossAxis boxInsets)

containerContentSize :: Direction -> Double -> [Size] -> Size
containerContentSize direction gap sizes =
  sizeFromAxes primaryAxis primarySize crossSize
  where
    primaryAxis = directionAxis direction
    crossAxis = otherAxis primaryAxis
    primarySize = clayAdd (claySum (axisSize primaryAxis <$> sizes)) (gapSize gap sizes)
    crossSize = maximumOrZero (axisSize crossAxis <$> sizes)

closeNodeSizing :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNodeSizing node =
  node
    { nodeDimensions = mapSizeAxes (\axis -> closeAxisSize (axisSizing axis sizing)) dimensions
    , nodeMinDimensions = mapSizeAxes (\axis -> closeAxisMinSize (axisSizing axis sizing)) minDimensions
    }
  where
    sizing = nodeSizing node
    dimensions = nodeDimensions node
    minDimensions = nodeMinDimensions node

updateMissingAspectDimension :: LayoutNode measureM placed -> LayoutNode measureM placed
updateMissingAspectDimension node =
  case nodeAspectRatio node of
    Just ratio
      | ratio /= 0 ->
          node {nodeDimensions = fillMissingAspectDimension ratio (nodeDimensions node)}
    _ -> node

clampNodeHeight :: LayoutNode measureM placed -> Double -> Double
clampNodeHeight node value =
  clampMax (nodeAxisMax Vertical node) (clampMin (nodeHeightMinForPropagation node) value)
  where
    clampMin minimumValue = max minimumValue
    clampMax maximumValue = min maximumValue

-- Clay's sizing union overlays percent on minMax.min, so its height
-- propagation clamps percent-sized parents to the raw percent fraction
-- (and to a zero max). Removing this pun visibly diverges from the oracle:
-- Clay collapses such boxes, e.g. to 0x0, where the natural rule would not.
nodeHeightMinForPropagation :: LayoutNode measureM placed -> Double
nodeHeightMinForPropagation node =
  case percentValue sizing of
    Just percent -> percent
    Nothing -> axisMin sizing
  where
    sizing = sizingHeight (nodeSizing node)

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
placeTextNode _ LayoutNode {nodeContent = Container} =
  pure mempty
placeTextNode _ LayoutNode {nodeContent = Intrinsic _} =
  pure mempty
placeTextNode Point {pointX, pointY} LayoutNode {nodeContent = TextContent textNode, nodeDimensions = Size {sizeWidth}} =
  snd <$> foldM placeLine (pointY + lineHeightOffset, mempty) (zip [0 ..] (textNodeLines textNode))
  where
    lineHeight = textNodeLineHeight textNode
    lineHeightOffset = (lineHeight - textNodeNaturalLineHeight textNode) / 2
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
    primaryAxis = directionAxis (boxDirection config)
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
    childOffset = nodeChildOffset node
    placeChild (primaryPosition, placed) child = do
      let childSize = nodeDimensions child
      let childPrimary = axisSize primaryAxis childSize
      let childCross = axisSize crossAxis childSize
      let crossPosition =
            rectAxisPosition crossAxis inner
              + crossAlignmentOffset (boxCrossAlign config) (axisSize crossAxis (shrinkSize boxInsets (nodeDimensions node))) childCross
      next <- placeLayoutNode (offsetPoint childOffset (pointFromAxes primaryAxis primaryPosition crossPosition)) child
      pure (primaryPosition + childPrimary + boxGap config, placed <> next)

measureTextNode :: Monad measureM => TextConfig measureM placed -> String -> measureM (TextNode measureM placed)
measureTextNode config string = do
  spaceSize <- textMeasure config " "
  measurement <- measureTextTokens config string (sizeWidth spaceSize)
  pure
    TextNode
      { textNodeText = string
      , textNodeConfig = config
      , textNodePreferredSize = Size (measurementWidth measurement) (lineHeight measurement)
      , textNodeNaturalLineHeight = measurementHeight measurement
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

-- Mirrors Clay's measure scanner: every space ends a token (so consecutive
-- spaces produce empty words), and newlines break words without a space.
data TextSegment = TextSegment
  { segmentWord :: String
  , segmentBreak :: SegmentBreak
  }

data SegmentBreak
  = BreakSpace
  | BreakNewline
  | BreakEnd

segmentText :: String -> [TextSegment]
segmentText string =
  case break (`elem` " \n") string of
    (word, ' ' : rest) -> TextSegment word BreakSpace : segmentText rest
    (word, '\n' : rest) -> TextSegment word BreakNewline : segmentText rest
    (word, _) -> [TextSegment word BreakEnd]

data MeasuredSegment = MeasuredSegment
  { measuredSegmentSize :: Size
  , measuredSegment :: TextSegment
  }

measureTextTokens :: Monad measureM => TextConfig measureM placed -> String -> Double -> measureM TextMeasurement
measureTextTokens config string spaceWidth = do
  measuredSegments <- traverse measureSegment (segmentText string)
  let tokens = concatMap segmentTokens measuredSegments
  pure
    TextMeasurement
      { measurementWidth = maximumOrZero (tokenLineWidths tokens)
      , measurementHeight = maximumOrZero (sizeHeight . measuredSegmentSize <$> measuredSegments)
      , measurementMinWidth = maximumOrZero (sizeWidth . measuredSegmentSize <$> measuredSegments)
      , measurementTokens = tokens
      , measurementContainsNewlines = any (segmentBreaksLine . measuredSegment) measuredSegments
      }
  where
    measureSegment segment@TextSegment {segmentWord}
      | null segmentWord = pure (MeasuredSegment (Size 0 0) segment)
      | otherwise = (`MeasuredSegment` segment) <$> textMeasure config segmentWord
    segmentTokens MeasuredSegment {measuredSegmentSize, measuredSegment = TextSegment {segmentWord, segmentBreak}} =
      case segmentBreak of
        BreakSpace -> [TextWord (segmentWord <> " ") (sizeWidth measuredSegmentSize + spaceWidth)]
        BreakNewline -> wordToken <> [TextNewline]
        BreakEnd -> wordToken
      where
        wordToken = [TextWord segmentWord (sizeWidth measuredSegmentSize) | not (null segmentWord)]
    segmentBreaksLine TextSegment {segmentBreak} =
      case segmentBreak of
        BreakNewline -> True
        _ -> False

tokenLineWidths :: [TextToken] -> [Double]
tokenLineWidths tokens =
  sum [tokenWidth | TextWord _ tokenWidth <- line] : remainingWidths
  where
    (line, rest) = break isTextNewline tokens
    remainingWidths =
      case rest of
        _newline : remaining -> tokenLineWidths remaining
        [] -> []

isTextNewline :: TextToken -> Bool
isTextNewline TextNewline = True
isTextNewline _ = False

wrapTextNode :: Double -> TextNode measureM placed -> TextNode measureM placed
wrapTextNode availableWidth textNode =
  textNode {textNodeLines = linesForMode}
  where
    linesForMode =
      case textWrapMode (textNodeConfig textNode) of
        TextWrapWords ->
          if not (textNodeContainsNewlines textNode) && sizeWidth (textNodePreferredSize textNode) <= availableWidth
            then [TextLine (textNodeText textNode) (sizeWidth (textNodePreferredSize textNode))]
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
      case reverse currentText of
        ' ' : reversedTrimmed -> TextLine (reverse reversedTrimmed) (currentWidth - spaceWidth)
        _ -> TextLine currentText currentWidth

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

data Axis
  = Horizontal
  | Vertical
  deriving (Eq)

otherAxis :: Axis -> Axis
otherAxis Horizontal = Vertical
otherAxis Vertical = Horizontal

directionAxis :: Direction -> Axis
directionAxis LeftToRight = Horizontal
directionAxis TopToBottom = Vertical

axisSize :: Axis -> Size -> Double
axisSize Horizontal = sizeWidth
axisSize Vertical = sizeHeight

setSizeAxis :: Axis -> Double -> Size -> Size
setSizeAxis Horizontal value size =
  size {sizeWidth = normalizeClayDimension value}
setSizeAxis Vertical value size =
  size {sizeHeight = normalizeClayDimension value}

sizeFromAxes :: Axis -> Double -> Double -> Size
sizeFromAxes Horizontal primarySize crossSize =
  Size primarySize crossSize
sizeFromAxes Vertical primarySize crossSize =
  Size crossSize primarySize

mapSizeAxes :: (Axis -> Double -> Double) -> Size -> Size
mapSizeAxes change size =
  Size
    { sizeWidth = change Horizontal (sizeWidth size)
    , sizeHeight = change Vertical (sizeHeight size)
    }

axisSizing :: Axis -> Sizing -> AxisSizing
axisSizing Horizontal = sizingWidth
axisSizing Vertical = sizingHeight

updateNodeAxisDimension :: Axis -> Double -> LayoutNode measureM placed -> LayoutNode measureM placed
updateNodeAxisDimension axis value node =
  node {nodeDimensions = setSizeAxis axis value (nodeDimensions node)}

boxClipsAxis :: Axis -> BoxConfig -> Bool
boxClipsAxis Horizontal config =
  clipHorizontal (boxClip config)
boxClipsAxis Vertical config =
  clipVertical (boxClip config)

nodeClipsAxis :: Axis -> LayoutNode measureM placed -> Bool
nodeClipsAxis axis node =
  boxClipsAxis axis (nodeConfig node)

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
axisPadding Horizontal Insets {insetLeft, insetRight} = clayAdd insetLeft insetRight
axisPadding Vertical Insets {insetTop, insetBottom} = clayAdd insetTop insetBottom

shrinkSize :: Insets -> Size -> Size
shrinkSize Insets {insetTop, insetRight, insetBottom, insetLeft} Size {sizeWidth, sizeHeight} =
  Size
    { sizeWidth = claySub (claySub sizeWidth insetLeft) insetRight
    , sizeHeight = claySub (claySub sizeHeight insetTop) insetBottom
    }

closeAxisSize :: AxisSizing -> Double -> Double
closeAxisSize Percent {} _ = 0
closeAxisSize sizing value = normalizeClayDimension (clampAxis sizing value)

closeAxisMinSize :: AxisSizing -> Double -> Double
closeAxisMinSize Percent {} value = value
closeAxisMinSize sizing value = normalizeClayDimension (clampAxis sizing value)

data DistributionMode
  = GrowNodes
  | CompressNodes

distributionComplete :: DistributionMode -> Double -> Bool
distributionComplete GrowNodes remaining =
  remaining <= clayEpsilon
distributionComplete CompressNodes remaining =
  remaining >= negate clayEpsilon

distributionBound :: DistributionMode -> Axis -> LayoutNode measureM placed -> Double
distributionBound GrowNodes axis node =
  nodeAxisMax axis node
distributionBound CompressNodes axis node =
  axisSize axis (nodeMinDimensions node)

applyDistribution :: DistributionMode -> Double -> Double -> Double -> Double
applyDistribution GrowNodes previousSize resizeAmount bound =
  min (clayAdd previousSize resizeAmount) bound
applyDistribution CompressNodes previousSize resizeAmount bound =
  max (clayAdd previousSize resizeAmount) bound

distributionAtBound :: DistributionMode -> Double -> Double -> Bool
distributionAtBound GrowNodes size bound =
  size >= bound
distributionAtBound CompressNodes size bound =
  size <= bound

isFill :: AxisSizing -> Bool
isFill Fill {} = True
isFill _ = False

isFixed :: AxisSizing -> Bool
isFixed Fixed {} = True
isFixed _ = False

isPercent :: AxisSizing -> Bool
isPercent Percent {} = True
isPercent _ = False

textNodeCanResize :: LayoutNode measureM placed -> Bool
textNodeCanResize LayoutNode {nodeContent = TextContent textNode} =
  textWrapMode (textNodeConfig textNode) == TextWrapWords
textNodeCanResize _ = True

sizingMinMax :: AxisSizing -> MinMax
sizingMinMax (Fit minMax) = minMax
sizingMinMax (Fill minMax) = minMax
sizingMinMax (Fixed value) = MinMax (Just value) (Just value)
sizingMinMax (Percent _) = unbounded

clampAxis :: AxisSizing -> Double -> Double
clampAxis sizing =
  clampMax . clampMin
  where
    MinMax maybeMin maybeMax = sizingMinMax sizing
    clampMin = maybe id max maybeMin
    clampMax = maybe id min maybeMax

axisMax :: AxisSizing -> Double
axisMax sizing =
  case sizingMinMax sizing of
    MinMax _ (Just maximumValue) -> maximumValue
    _ -> clayMaxFloat

axisMin :: AxisSizing -> Double
axisMin sizing =
  case sizingMinMax sizing of
    MinMax (Just minimumValue) _ -> minimumValue
    _ -> 0

-- More Clay sizing-union punning on the vertical axis: a percent sizing
-- leaves the overlaid minMax.max bytes zeroed, and the aspect height pass
-- writes minMax.max through the union (nodeHeightMaxOverride here).
nodeAxisMax :: Axis -> LayoutNode measureM placed -> Double
nodeAxisMax Horizontal node =
  axisMax (sizingWidth (nodeSizing node))
nodeAxisMax Vertical node =
  case (isPercent sizing, nodeHeightMaxOverride node) of
    (True, Just maximumValue) -> maximumValue
    (True, Nothing) -> 0
    (_, Just maximumValue) -> maximumValue
    (_, Nothing) -> axisMax sizing
  where
    sizing = sizingHeight (nodeSizing node)

percentValue :: AxisSizing -> Maybe Double
percentValue (Percent value) = Just value
percentValue _ = Nothing

modifyAt :: Int -> (item -> item) -> [item] -> [item]
modifyAt index change items =
  take index items <> [change (items !! index)] <> drop (index + 1) items

removeSwapbackAt :: Int -> [item] -> [item]
removeSwapbackAt index items
  | index == lastIndex = take index items
  | otherwise = take index items <> [last items] <> drop (index + 1) (take lastIndex items)
  where
    lastIndex = length items - 1

-- Clay computes in float32. The sizing phases reproduce its rounding with
-- the clay* arithmetic below because epsilon comparisons there decide how
-- space is distributed; the placement phase stays in Double since positions
-- feed no further layout decisions.
clayEpsilon :: Double
clayEpsilon = 0.01

clayMaxFloat :: Double
clayMaxFloat = 3.4028234663852886e38

clayFloatEqual :: Double -> Double -> Bool
clayFloatEqual left right =
  difference < clayEpsilon && difference > -clayEpsilon
  where
    difference = left - right

normalizeClayDimension :: Double -> Double
normalizeClayDimension =
  clayFloatToDouble . doubleToClayFloat

doubleToClayFloat :: Double -> Float
doubleToClayFloat =
  realToFrac

clayFloatToDouble :: Float -> Double
clayFloatToDouble =
  realToFrac

clayAdd :: Double -> Double -> Double
clayAdd left right =
  clayFloatToDouble (doubleToClayFloat left + doubleToClayFloat right)

claySub :: Double -> Double -> Double
claySub left right =
  clayFloatToDouble (doubleToClayFloat left - doubleToClayFloat right)

clayMul :: Double -> Double -> Double
clayMul left right =
  clayFloatToDouble (doubleToClayFloat left * doubleToClayFloat right)

clayDiv :: Double -> Double -> Double
clayDiv left right =
  clayFloatToDouble (doubleToClayFloat left / doubleToClayFloat right)

claySum :: [Double] -> Double
claySum =
  foldl clayAdd 0

clayExpandSize :: Insets -> Size -> Size
clayExpandSize Insets {insetTop, insetRight, insetBottom, insetLeft} Size {sizeWidth, sizeHeight} =
  Size
    { sizeWidth = clayAdd (clayAdd sizeWidth insetLeft) insetRight
    , sizeHeight = clayAdd (clayAdd sizeHeight insetTop) insetBottom
    }

fillMissingAspectDimension :: Double -> Size -> Size
fillMissingAspectDimension ratio size@Size {sizeWidth, sizeHeight}
  | ratio == 0 = size
  | dimensionIsMissing sizeWidth && not (dimensionIsMissing sizeHeight) = Size (clayMul sizeHeight ratio) sizeHeight
  | not (dimensionIsMissing sizeWidth) && dimensionIsMissing sizeHeight = Size sizeWidth (clayMul (clayDiv 1 ratio) sizeWidth)
  | otherwise = size

dimensionIsMissing :: Double -> Bool
dimensionIsMissing value =
  value == 0

mainAlignmentOffset :: MainAlign -> Double -> Double
mainAlignmentOffset MainStart _extra = 0
mainAlignmentOffset MainCenter extra = extra / 2
mainAlignmentOffset MainEnd extra = extra

crossAlignmentOffset :: CrossAlign -> Double -> Double -> Double
crossAlignmentOffset CrossStart _available _child = 0
crossAlignmentOffset CrossCenter available child = (available - child) / 2
crossAlignmentOffset CrossEnd available child = available - child

gapSize :: Double -> [item] -> Double
gapSize gap items =
  clayMul gap (fromIntegral (max 0 (length items - 1)))

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
