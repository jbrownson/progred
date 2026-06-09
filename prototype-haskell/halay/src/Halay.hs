module Halay
  ( AxisSizing (..)
  , BoxConfig (..)
  , BoxClip (..)
  , CrossAlign (..)
  , Direction (..)
  , Halay
  , MainAlign (..)
  , Measured (..)
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

data FlatNode measureM placed = FlatNode
  { flatNodeLayout :: LayoutNode measureM placed
  , flatNodeChildren :: [Int]
  }

data FlatLayout measureM placed = FlatLayout
  { flatLayoutNodes :: [FlatNode measureM placed]
  , flatLayoutRoot :: Int
  }

data SizePhaseResult measureM placed = SizePhaseResult
  { sizePhaseLayout :: FlatLayout measureM placed
  , sizePhaseTextNodes :: [Int]
  , sizePhaseAspectNodes :: [Int]
  }

defaultSizing :: Sizing
defaultSizing = Sizing Fit Fit

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
  rebuildFlatLayout
    . layoutFlat
    . flattenLayout
    . overrideRootSize rootSizeOverride
    . closeNode

flattenLayout :: LayoutNode measureM placed -> FlatLayout measureM placed
flattenLayout root =
  FlatLayout
    { flatLayoutNodes = nodes
    , flatLayoutRoot = rootIndex
    }
  where
    (_nextIndex, nodes, rootIndex) = flattenFrom 0 root

flattenFrom :: Int -> LayoutNode measureM placed -> (Int, [FlatNode measureM placed], Int)
flattenFrom index node =
  (nextIndex, ownNode : concat childNodeLists, index)
  where
    (nextIndex, childIndicesReversed, childNodeListsReversed) =
      foldl flattenChild (index + 1, [], []) (nodeChildren node)
    flattenChild (nextChildIndex, childIndicesSoFar, childNodeListsSoFar) child =
      (nextAfterChild, childIndex : childIndicesSoFar, childNodes : childNodeListsSoFar)
      where
        (nextAfterChild, childNodes, childIndex) = flattenFrom nextChildIndex child
    childIndices = reverse childIndicesReversed
    childNodeLists = reverse childNodeListsReversed
    ownNode =
      FlatNode
        { flatNodeLayout = node {nodeChildren = []}
        , flatNodeChildren = childIndices
        }

rebuildFlatLayout :: FlatLayout measureM placed -> LayoutNode measureM placed
rebuildFlatLayout FlatLayout {flatLayoutNodes, flatLayoutRoot} =
  rebuild flatLayoutRoot
  where
    rebuild index =
      layout {nodeChildren = rebuild <$> flatNodeChildren flatNode}
      where
        flatNode = flatLayoutNodes !! index
        layout = flatNodeLayout flatNode

layoutFlat :: FlatLayout measureM placed -> FlatLayout measureM placed
layoutFlat source =
  scaleAspectWidthsFlat aspectNodes afterVerticalSizing
  where
    horizontalResult = sizeContainersAlongAxisFlat Horizontal True source
    afterHorizontalSizing = sizePhaseLayout horizontalResult
    textNodes = sizePhaseTextNodes horizontalResult
    aspectNodes = sizePhaseAspectNodes horizontalResult
    wrappedText = wrapTextNodesFlat textNodes afterHorizontalSizing
    aspectHeights = scaleAspectHeightsFlat aspectNodes wrappedText
    propagatedHeights = propagateResolvedHeightsFlat aspectHeights
    afterVerticalSizing = sizePhaseLayout (sizeContainersAlongAxisFlat Vertical False propagatedHeights)

flatNodeAt :: Int -> FlatLayout measureM placed -> FlatNode measureM placed
flatNodeAt index FlatLayout {flatLayoutNodes} =
  flatLayoutNodes !! index

flatLayoutNodeAt :: Int -> FlatLayout measureM placed -> LayoutNode measureM placed
flatLayoutNodeAt index =
  flatNodeLayout . flatNodeAt index

modifyFlatNode :: Int -> (FlatNode measureM placed -> FlatNode measureM placed) -> FlatLayout measureM placed -> FlatLayout measureM placed
modifyFlatNode index change layout@FlatLayout {flatLayoutNodes} =
  layout {flatLayoutNodes = replaceAt index (change (flatLayoutNodes !! index)) flatLayoutNodes}

modifyFlatLayoutNode :: Int -> (LayoutNode measureM placed -> LayoutNode measureM placed) -> FlatLayout measureM placed -> FlatLayout measureM placed
modifyFlatLayoutNode index change =
  modifyFlatNode index changeFlatNode
  where
    changeFlatNode flatNode =
      flatNode {flatNodeLayout = change (flatNodeLayout flatNode)}

sizeContainersAlongAxisFlat :: Axis -> Bool -> FlatLayout measureM placed -> SizePhaseResult measureM placed
sizeContainersAlongAxisFlat axis collectPhaseNodes source =
  sizeBreadthFirst [flatLayoutRoot source] (SizePhaseResult source [] [])
  where
    sizeBreadthFirst [] result = result
    sizeBreadthFirst (parentIndex : remaining) result =
      sizeBreadthFirst
        (remaining <> parentSizingQueuedChildren parentSizing)
        SizePhaseResult
          { sizePhaseLayout = parentSizingLayout parentSizing
          , sizePhaseTextNodes = sizePhaseTextNodes result <> parentSizingTextNodes parentSizing
          , sizePhaseAspectNodes = sizePhaseAspectNodes result <> parentSizingAspectNodes parentSizing
          }
      where
        parentSizing =
          sizeFlatParent axis collectPhaseNodes parentIndex (sizePhaseLayout result)

data ParentSizing measureM placed = ParentSizing
  { parentSizingLayout :: FlatLayout measureM placed
  , parentSizingQueuedChildren :: [Int]
  , parentSizingTextNodes :: [Int]
  , parentSizingAspectNodes :: [Int]
  }

data ParentScan = ParentScan
  { scanInnerContentSize :: Double
  , scanTotalPaddingAndChildGaps :: Double
  , scanGrowContainerCount :: Int
  , scanResizableChildren :: [Int]
  , scanQueuedChildren :: [Int]
  , scanTextNodes :: [Int]
  , scanAspectNodes :: [Int]
  , scanIsFirstChild :: Bool
  }

sizeFlatParent :: Axis -> Bool -> Int -> FlatLayout measureM placed -> ParentSizing measureM placed
sizeFlatParent axis collectPhaseNodes parentIndex source =
  ParentSizing
    { parentSizingLayout = distributeParentSpace percentLayout percentInnerContentSize
    , parentSizingQueuedChildren = scanQueuedChildren scan
    , parentSizingTextNodes = scanTextNodes scan
    , parentSizingAspectNodes = scanAspectNodes scan
    }
  where
    parentFlatNode = flatNodeAt parentIndex source
    parent = flatNodeLayout parentFlatNode
    parentChildren = flatNodeChildren parentFlatNode
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
        , scanQueuedChildren = []
        , scanTextNodes = []
        , scanAspectNodes = []
        , scanIsFirstChild = True
        }
    scan = foldl scanChild initialScan parentChildren
    scanChild current childIndex =
      current
        { scanInnerContentSize = nextInnerContentSize
        , scanTotalPaddingAndChildGaps = nextTotalPaddingAndChildGaps
        , scanGrowContainerCount = nextGrowContainerCount
        , scanResizableChildren = scanResizableChildren current <> resizableChild
        , scanQueuedChildren = scanQueuedChildren current <> queuedChild
        , scanTextNodes = scanTextNodes current <> textNode
        , scanAspectNodes = scanAspectNodes current <> aspectNode
        , scanIsFirstChild = False
        }
      where
        childFlatNode = flatNodeAt childIndex source
        child = flatNodeLayout childFlatNode
        childSizing = nodeAxisSizing axis child
        childSize = axisSize axis (nodeDimensions child)
        childGap =
          if scanIsFirstChild current || not sizingAlongAxis
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
        queuedChild =
          [ childIndex
          | not (nodeIsText child)
          , not (null (flatNodeChildren childFlatNode))
          ]
        textNode =
          [ childIndex
          | collectPhaseNodes
          , nodeIsText child
          ]
        aspectNode =
          [ childIndex
          | collectPhaseNodes
          , not (nodeIsText child)
          , nodeHasAspectRatio child
          ]
    (percentLayout, percentInnerContentSize) =
      foldl expandPercentChild (source, scanInnerContentSize scan) parentChildren
    expandPercentChild (currentLayout, innerContentSize) childIndex =
      case nodeAxisSizing axis child of
        Percent percent ->
          ( updateMissingAspectFlat childIndex $
              modifyFlatLayoutNode childIndex (updateNodeAxisDimension axis percentSize) currentLayout
          , if sizingAlongAxis then clayAdd innerContentSize percentSize else innerContentSize
          )
          where
            percentSize = clayMul (claySub parentSize (scanTotalPaddingAndChildGaps scan)) percent
        _ -> (currentLayout, innerContentSize)
      where
        child = flatLayoutNodeAt childIndex currentLayout
    distributeParentSpace currentLayout innerContentSize
      | sizingAlongAxis && sizeToDistribute < 0 && nodeClipsAxis axis parent =
          currentLayout
      | sizingAlongAxis && sizeToDistribute < 0 =
          distributeFlatNodes CompressNodes axis sizeToDistribute currentLayout (scanResizableChildren scan)
      | sizingAlongAxis && sizeToDistribute > 0 && scanGrowContainerCount scan > 0 =
          distributeFlatNodes GrowNodes axis sizeToDistribute currentLayout growChildren
      | sizingAlongAxis =
          currentLayout
      | otherwise =
          resolveFlatCrossAxisChildren axis parent parentSize parentPadding innerContentSize currentLayout (scanResizableChildren scan)
      where
        sizeToDistribute = claySub (claySub parentSize parentPadding) innerContentSize
        growChildren =
          filter
            (\childIndex -> isFill (nodeAxisSizing axis (flatLayoutNodeAt childIndex currentLayout)))
            (scanResizableChildren scan)

nodeAxisSizing :: Axis -> LayoutNode measureM placed -> AxisSizing
nodeAxisSizing axis =
  axisSizing axis . nodeSizing

nodeIsText :: LayoutNode measureM placed -> Bool
nodeIsText LayoutNode {nodeContent = TextContent _} = True
nodeIsText _ = False

nodeHasAspectRatio :: LayoutNode measureM placed -> Bool
nodeHasAspectRatio node =
  case nodeAspectRatio node of
    Just ratio -> ratio /= 0
    Nothing -> False

updateMissingAspectFlat :: Int -> FlatLayout measureM placed -> FlatLayout measureM placed
updateMissingAspectFlat index =
  modifyFlatLayoutNode index updateMissingAspectDimension

resolveFlatCrossAxisChildren :: Axis -> LayoutNode measureM placed -> Double -> Double -> Double -> FlatLayout measureM placed -> [Int] -> FlatLayout measureM placed
resolveFlatCrossAxisChildren axis parent parentSize parentPadding innerContentSize =
  foldl resolve
  where
    maxSize
      | nodeClipsAxis axis parent = max visibleMaxSize innerContentSize
      | otherwise = visibleMaxSize
    visibleMaxSize = claySub parentSize parentPadding
    resolve currentLayout childIndex =
      modifyFlatLayoutNode childIndex resize currentLayout
      where
        child = flatLayoutNodeAt childIndex currentLayout
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

distributeFlatNodes :: DistributionMode -> Axis -> Double -> FlatLayout measureM placed -> [Int] -> FlatLayout measureM placed
distributeFlatNodes mode axis remaining layout activeIndices
  | distributionComplete mode remaining = layout
  | null activeIndices = layout
  | otherwise =
      distributeFlatNodes mode axis remainingAfterPass layoutAfterPass activeAfterPass
  where
    (frontierSize, resizeAmount) = flatDistributionStep mode axis remaining layout activeIndices
    (remainingAfterPass, layoutAfterPass, activeAfterPass) =
      applyFlatDistributionPass mode axis frontierSize resizeAmount remaining layout activeIndices

applyFlatDistributionPass :: DistributionMode -> Axis -> Double -> Double -> Double -> FlatLayout measureM placed -> [Int] -> (Double, FlatLayout measureM placed, [Int])
applyFlatDistributionPass mode axis frontierSize resizeAmount =
  step 0
  where
    step position remaining layout activeIndices
      | position >= length activeIndices = (remaining, layout, activeIndices)
      | not (clayFloatEqual previousSize frontierSize) =
          step (position + 1) remaining layout activeIndices
      | otherwise =
          step nextPosition nextRemaining nextLayout nextActiveIndices
      where
        childIndex = activeIndices !! position
        child = flatLayoutNodeAt childIndex layout
        previousSize = axisSize axis (nodeDimensions child)
        bound = distributionBound mode axis child
        newSize = applyDistribution mode previousSize resizeAmount bound
        nextRemaining = claySub remaining (claySub newSize previousSize)
        nextLayout = modifyFlatLayoutNode childIndex (updateNodeAxisDimension axis newSize) layout
        atBound = distributionAtBound mode newSize bound
        nextActiveIndices =
          if atBound
            then removeSwapbackAt position activeIndices
            else activeIndices
        nextPosition =
          if atBound
            then position
            else position + 1

flatDistributionStep :: DistributionMode -> Axis -> Double -> FlatLayout measureM placed -> [Int] -> (Double, Double)
flatDistributionStep GrowNodes axis remaining layout activeIndices =
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
        childSize = axisSize axis (nodeDimensions (flatLayoutNodeAt childIndex layout))
flatDistributionStep CompressNodes axis remaining layout activeIndices =
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
        childSize = axisSize axis (nodeDimensions (flatLayoutNodeAt childIndex layout))

wrapTextNodesFlat :: [Int] -> FlatLayout measureM placed -> FlatLayout measureM placed
wrapTextNodesFlat indices layout =
  foldl (\current index -> modifyFlatLayoutNode index wrap current) layout indices
  where
    wrap node =
      node
        { nodeContent = wrappedContent
        , nodeDimensions = wrappedDimensions
        }
      where
        wrappedContent =
          case nodeContent node of
            TextContent textNode -> TextContent (wrapTextNode (sizeWidth (nodeDimensions node)) textNode)
            other -> other
        wrappedDimensions =
          case wrappedContent of
            TextContent textNode ->
              (nodeDimensions node) {sizeHeight = clayMul (textNodeLineHeight textNode) (fromIntegral (length (textNodeLines textNode)))}
            _ -> nodeDimensions node

scaleAspectHeightsFlat :: [Int] -> FlatLayout measureM placed -> FlatLayout measureM placed
scaleAspectHeightsFlat indices layout =
  foldl (\current index -> modifyFlatLayoutNode index adjust current) layout indices
  where
    adjust node =
      case nodeAspectRatio node of
        Just ratio
          | ratio /= 0 ->
              node
                { nodeDimensions = setSizeAxis Vertical aspectHeight (nodeDimensions node)
                , nodeHeightMaxOverride = Just aspectHeight
                }
          where
            aspectHeight = clayDiv (sizeWidth (nodeDimensions node)) ratio
        _ -> node

scaleAspectWidthsFlat :: [Int] -> FlatLayout measureM placed -> FlatLayout measureM placed
scaleAspectWidthsFlat indices layout =
  foldl (\current index -> modifyFlatLayoutNode index adjust current) layout indices
  where
    adjust node =
      case nodeAspectRatio node of
        Just ratio ->
          updateNodeAxisDimension Horizontal (clayMul ratio (sizeHeight (nodeDimensions node))) node
        Nothing -> node

propagateResolvedHeightsFlat :: FlatLayout measureM placed -> FlatLayout measureM placed
propagateResolvedHeightsFlat layout =
  foldl propagateOne layout (reverse [0 .. length (flatLayoutNodes layout) - 1])
  where
    propagateOne current index
      | nodeIsText node = current
      | null children = current
      | boxDirection config == LeftToRight =
          modifyFlatLayoutNode index resizeRow current
      | otherwise =
          modifyFlatLayoutNode index resizeColumn current
      where
        flatNode = flatNodeAt index current
        node = flatNodeLayout flatNode
        children = flatNodeChildren flatNode
        config = nodeConfig node
        resizeRow currentNode =
          foldl resizeForChild currentNode children
        resizeForChild currentNode childIndex =
          currentNode
            { nodeDimensions =
                (nodeDimensions currentNode)
                  { sizeHeight =
                      normalizeClayDimension $
                        clampNodeHeight currentNode $
                          max
                            (sizeHeight (nodeDimensions currentNode))
                            ( clayAdd
                                (clayAdd (sizeHeight (nodeDimensions (flatLayoutNodeAt childIndex current))) (insetTop (boxPadding config)))
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
                                (claySum [sizeHeight (nodeDimensions (flatLayoutNodeAt childIndex current)) | childIndex <- children])
                            )
                            (gapSize (boxGap config) children)
                  }
            }

overrideRootSize :: Maybe Size -> LayoutNode measureM placed -> LayoutNode measureM placed
overrideRootSize Nothing node = node
overrideRootSize (Just size) node = node {nodeDimensions = size}

closeNode :: LayoutNode measureM placed -> LayoutNode measureM placed
closeNode node =
  closeNodeAspect (closeNodeSizing closed)
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
closeAxisSize sizing value =
  case stripClamp sizing of
    Percent {} -> 0
    _ -> normalizeClayDimension (clampAxis sizing value)

closeAxisMinSize :: AxisSizing -> Double -> Double
closeAxisMinSize sizing value =
  case stripClamp sizing of
    Percent {} -> value
    _ -> normalizeClayDimension (clampAxis sizing value)

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

textNodeCanResize :: LayoutNode measureM placed -> Bool
textNodeCanResize LayoutNode {nodeContent = TextContent textNode} =
  textWrapMode (textNodeConfig textNode) == TextWrapWords
textNodeCanResize _ = True

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
percentValue (Clamp _ _ sizing) = percentValue sizing
percentValue (Percent value) = Just value
percentValue _ = Nothing

replaceAt :: Int -> item -> [item] -> [item]
replaceAt index value items =
  take index items <> [value] <> drop (index + 1) items

removeSwapbackAt :: Int -> [item] -> [item]
removeSwapbackAt index items
  | index == lastIndex = take index items
  | otherwise = take index items <> [last items] <> drop (index + 1) (take lastIndex items)
  where
    lastIndex = length items - 1

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
  | not (dimensionIsMissing sizeWidth) && dimensionIsMissing sizeHeight = Size sizeWidth (clayDiv sizeWidth ratio)
  | otherwise = size

dimensionIsMissing :: Double -> Bool
dimensionIsMissing value =
  value == 0

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
