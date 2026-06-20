module Progred.Render.Graph
  ( GraphEdge (..)
  , GraphLayout (..)
  , GraphNode (..)
  , GraphNodeKey (..)
  , GraphSelectionStrength (..)
  , GraphSnapshot (..)
  , GraphSelectedEdge (..)
  , GraphSelectedNode (..)
  , emptyGraphLayout
  , graphLayoutNodeCount
  , graphPanel
  , graphSnapshot
  , stepGraphLayout
  ) where

import Control.Applicative ((<|>))
import Data.Bits ((.&.), shiftR, xor)
import Data.Char (ord)
import Data.Functor ((<&>))
import Data.List (sortOn)
import qualified Data.Map.Strict as Map
import Data.Map.Strict (Map)
import qualified Data.Set as Set
import qualified Data.UUID.Types as UUID
import Data.Word (Word32)
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.Widgets.Identicon
import qualified Puri.Canvas as Canvas
import Puri.Handler

data GraphNodeKey
  = GraphRoot
  | GraphUUID UUID
  | GraphScalar String
  deriving (Eq, Ord, Show)

data GraphNode = GraphNode
  { graphNodeKey :: GraphNodeKey
  , graphNodeUUID :: Maybe UUID
  , graphNodeTitle :: Maybe String
  , graphNodeRoot :: Bool
  }
  deriving (Eq, Show)

data GraphEdge = GraphEdge
  { graphEdgeSource :: GraphNodeKey
  , graphEdgeLabel :: UUID
  , graphEdgeTarget :: GraphNodeKey
  , graphEdgeTitle :: Maybe String
  }
  deriving (Eq, Show)

data GraphSelectionStrength
  = GraphSelectionPrimary
  | GraphSelectionSecondary
  deriving (Eq, Show)

data GraphSelectedNode = GraphSelectedNode
  { graphSelectedNodeKey :: GraphNodeKey
  , graphSelectedNodeStrength :: GraphSelectionStrength
  }
  deriving (Eq, Show)

data GraphSelectedEdge = GraphSelectedEdge
  { graphSelectedEdgeSource :: GraphNodeKey
  , graphSelectedEdgeLabel :: UUID
  , graphSelectedEdgeStrength :: GraphSelectionStrength
  }
  deriving (Eq, Show)

data GraphSnapshot = GraphSnapshot
  { graphSnapshotNodes :: [GraphNode]
  , graphSnapshotEdges :: [GraphEdge]
  , graphSnapshotSelectedNode :: Maybe GraphSelectedNode
  , graphSnapshotSelectedEdge :: Maybe GraphSelectedEdge
  }
  deriving (Eq, Show)

data GraphLayout = GraphLayout
  { graphLayoutPositions :: Map GraphNodeKey Point
  , graphLayoutVelocities :: Map GraphNodeKey Point
  }
  deriving (Eq, Show)

emptyGraphLayout :: GraphLayout
emptyGraphLayout =
  GraphLayout
    { graphLayoutPositions = Map.empty
    , graphLayoutVelocities = Map.empty
    }

graphLayoutNodeCount :: GraphLayout -> Int
graphLayoutNodeCount =
  Map.size . graphLayoutPositions

graphSnapshot :: Editor -> GraphSnapshot
graphSnapshot editor =
  documentGraphSnapshot (editorDocument editor) (editorFocus editor)

documentGraphSnapshot :: Document -> Maybe Focus -> GraphSnapshot
documentGraphSnapshot document focus =
  GraphSnapshot
    { graphSnapshotNodes = orderedNodes nodes
    , graphSnapshotEdges = edges
    , graphSnapshotSelectedNode = selectedNode
    , graphSnapshotSelectedEdge = selectedEdge
    }
  where
    graph = documentGraph document
    context = documentContext document []
    (rootNodes, rootKey) =
      case documentRoot document of
        Nothing -> (Map.singleton GraphRoot (GraphNode GraphRoot Nothing (Just "root") True), Just GraphRoot)
        Just rootValue ->
          let key = rootValueKey rootValue
           in (Map.singleton key (nodeForValue graph key rootValue True), Just key)
    graphNodes =
      Map.fromList
        [ (GraphUUID node, uuidNode graph node False)
        | node <- Map.keys graph
        ]
    (edgeNodes, edges) =
      foldl collectSource (Map.empty, []) (Map.toList graph)
    nodes =
      markRoot rootKey (Map.unionsWith mergeNode [rootNodes, graphNodes, edgeNodes])
    selectedNode =
      focusedNodeKey document context focus <&> \key ->
        GraphSelectedNode key GraphSelectionSecondary
    selectedEdge =
      focusedEdge context focus <&> \Edge {edgeSource, edgeLabel} ->
        GraphSelectedEdge (GraphUUID edgeSource) edgeLabel GraphSelectionSecondary

    collectSource (nodeMap, edgeList) (source, sourceEdges) =
      foldl (collectEdge source) (nodeMap, edgeList) (Map.toList sourceEdges)

    collectEdge source (nodeMap, edgeList) (label, value) =
      ( Map.insertWith mergeNode targetKey (nodeForValue graph targetKey value False) nodeMap
      , GraphEdge
          { graphEdgeSource = GraphUUID source
          , graphEdgeLabel = label
          , graphEdgeTarget = targetKey
          , graphEdgeTitle = uuidName graph label
          }
          : edgeList
      )
      where
        targetKey = edgeValueKey source label value

orderedNodes :: Map GraphNodeKey GraphNode -> [GraphNode]
orderedNodes nodes =
  roots <> rest
  where
    allNodes = Map.elems nodes
    roots = sortOn graphNodeKey [node | node <- allNodes, graphNodeRoot node]
    rest = sortOn graphNodeKey [node | node <- allNodes, not (graphNodeRoot node)]

markRoot :: Maybe GraphNodeKey -> Map GraphNodeKey GraphNode -> Map GraphNodeKey GraphNode
markRoot maybeRootKey nodes =
  case maybeRootKey of
    Nothing -> nodes
    Just rootKey -> Map.adjust (\node -> node {graphNodeRoot = True}) rootKey nodes

mergeNode :: GraphNode -> GraphNode -> GraphNode
mergeNode new old =
  old
    { graphNodeTitle = graphNodeTitle old <|> graphNodeTitle new
    , graphNodeRoot = graphNodeRoot old || graphNodeRoot new
    }

uuidNode :: Map UUID Edges -> UUID -> Bool -> GraphNode
uuidNode graph node root =
  GraphNode
    { graphNodeKey = GraphUUID node
    , graphNodeUUID = Just node
    , graphNodeTitle = uuidName graph node
    , graphNodeRoot = root
    }

nodeForValue :: Map UUID Edges -> GraphNodeKey -> Value -> Bool -> GraphNode
nodeForValue graph key value root =
  case value of
    VRef node -> uuidNode graph node root
    _ ->
      GraphNode
        { graphNodeKey = key
        , graphNodeUUID = Nothing
        , graphNodeTitle = Just (valueTitle value)
        , graphNodeRoot = root
        }

uuidName :: Map UUID Edges -> UUID -> Maybe String
uuidName graph node = do
  edges <- Map.lookup node graph
  case Map.lookup nameLabel edges of
    Just (VString name) -> Just name
    _ -> Nothing

rootValueKey :: Value -> GraphNodeKey
rootValueKey value =
  case value of
    VRef node -> GraphUUID node
    _ -> GraphScalar "root"

edgeValueKey :: UUID -> UUID -> Value -> GraphNodeKey
edgeValueKey source label value =
  case value of
    VRef node -> GraphUUID node
    _ -> GraphScalar (edgeScalarKey source label)

edgeScalarKey :: UUID -> UUID -> String
edgeScalarKey source label =
  UUID.toString source <> "/" <> UUID.toString label

valueTitle :: Value -> String
valueTitle value =
  case value of
    VRef node -> shortUUID node
    VString string -> "\"" <> truncateText 22 string <> "\""
    VInt integer -> show integer
    VFloat double -> show double

focusedNodeKey :: Document -> GraphContext -> Maybe Focus -> Maybe GraphNodeKey
focusedNodeKey document context focus = do
  Focus {focusPath} <- focus
  case resolvePath context focusPath of
    Just (VRef node) -> Just (GraphUUID node)
    Just value
      | null focusPath -> Just (rootValueKey value)
      | otherwise -> do
          Edge {edgeSource, edgeLabel} <- pathEdge context focusPath
          Just (edgeValueKey edgeSource edgeLabel value)
    Nothing
      | null focusPath && documentRoot document == Nothing -> Just GraphRoot
      | otherwise -> Nothing

focusedEdge :: GraphContext -> Maybe Focus -> Maybe Edge
focusedEdge context focus = do
  Focus {focusPath} <- focus
  pathEdge context focusPath

stepGraphLayout :: GraphSnapshot -> GraphLayout -> GraphLayout
stepGraphLayout snapshot layout =
  integrateForces snapshot synced
  where
    synced = syncGraphLayout snapshot layout

syncGraphLayout :: GraphSnapshot -> GraphLayout -> GraphLayout
syncGraphLayout snapshot layout =
  GraphLayout
    { graphLayoutPositions = positions
    , graphLayoutVelocities = velocities
    }
  where
    nodes = graphSnapshotNodes snapshot
    keys = graphNodeKey <$> nodes
    keySet = Set.fromList keys
    existingPositions = Map.filterWithKey (\key _ -> Set.member key keySet) (graphLayoutPositions layout)
    existingVelocities = Map.filterWithKey (\key _ -> Set.member key keySet) (graphLayoutVelocities layout)
    (positions, velocities) =
      foldl ensureNode (existingPositions, existingVelocities) (zip [0 :: Int ..] nodes)
    ensureNode (positionMap, velocityMap) (index, node) =
      ( Map.insertWith (\_ old -> old) key initialPosition positionMap
      , Map.insertWith (\_ old -> old) key (Point 0 0) velocityMap
      )
      where
        key = graphNodeKey node
        initialPosition =
          if Map.null positionMap && index == 0
            then Point 0 0
            else deterministicPosition key index

integrateForces :: GraphSnapshot -> GraphLayout -> GraphLayout
integrateForces snapshot layout =
  layout
    { graphLayoutPositions = positions
    , graphLayoutVelocities = velocities
    }
  where
    nodes = graphSnapshotNodes snapshot
    initialForces =
      Map.fromList [(graphNodeKey node, Point 0 0) | node <- nodes]
    repelledForces =
      applyRepulsion nodes (graphLayoutPositions layout) initialForces
    springForces =
      applySprings (graphSnapshotEdges snapshot) (graphLayoutPositions layout) repelledForces
    (positions, velocities) =
      foldl integrateNode (graphLayoutPositions layout, graphLayoutVelocities layout) nodes
    integrateNode (positionMap, velocityMap) node =
      ( Map.insert key (pointAdd position velocity) positionMap
      , Map.insert key velocity velocityMap
      )
      where
        key = graphNodeKey node
        position = Map.findWithDefault (Point 0 0) key positionMap
        currentVelocity = Map.findWithDefault (Point 0 0) key velocityMap
        force = pointAdd (Map.findWithDefault (Point 0 0) key springForces) (pointScale position (-gravityK))
        velocity = pointScale (pointAdd currentVelocity force) damping

applyRepulsion :: [GraphNode] -> Map GraphNodeKey Point -> Map GraphNodeKey Point -> Map GraphNodeKey Point
applyRepulsion nodes positions initialForces =
  foldl applyPair initialForces pairs
  where
    pairs =
      [ (graphNodeKey a, graphNodeKey b)
      | (index, a) <- zip [0 :: Int ..] nodes
      , b <- drop (index + 1) nodes
      ]
    applyPair currentForces (a, b) =
      case (Map.lookup a positions, Map.lookup b positions) of
        (Just pa, Just pb) ->
          let delta = pointSub pa pb
              force = pointScale (pointNormalize delta) (min maxForce (repulsionK / max 1 (pointLengthSq delta)))
           in addForce b (pointScale force (-1)) (addForce a force currentForces)
        _ -> currentForces

applySprings :: [GraphEdge] -> Map GraphNodeKey Point -> Map GraphNodeKey Point -> Map GraphNodeKey Point
applySprings edges positions initialForces =
  foldl applyEdge initialForces edges
  where
    applyEdge currentForces edge =
      case (Map.lookup (graphEdgeSource edge) positions, Map.lookup (graphEdgeTarget edge) positions) of
        (Just pa, Just pb) ->
          let delta = pointSub pb pa
              distance = max 0.1 (pointLength delta)
              magnitude = clamp (-maxForce) maxForce (attractionK * (distance - restLength))
              force = pointScale (pointNormalize delta) magnitude
           in addForce (graphEdgeTarget edge) (pointScale force (-1)) (addForce (graphEdgeSource edge) force currentForces)
        _ -> currentForces

addForce :: GraphNodeKey -> Point -> Map GraphNodeKey Point -> Map GraphNodeKey Point
addForce key force =
  Map.adjust (pointAdd force) key

graphPanel :: Canvas.Canvas renderM => GraphSnapshot -> GraphLayout -> Halay renderM renderM (Handler actionM)
graphPanel snapshot layout =
  leafWithSizing panelSizing (pure (Size 320 240)) draw
  where
    draw rect = do
      drawGraphPanel snapshot (syncGraphLayout snapshot layout) rect
      pure mempty

drawGraphPanel :: Canvas.Canvas renderM => GraphSnapshot -> GraphLayout -> Rect -> renderM ()
drawGraphPanel snapshot layout rect = do
  Canvas.fillRect rect panelBackground
  Canvas.strokeLine (Point (x rect) (y rect)) (Point (x rect) (y rect + height rect)) panelSeparator 1
  nodeSizes <- traverse nodeSize (graphSnapshotNodes snapshot)
  let sizesByKey = Map.fromList [(graphNodeKey node, size) | (node, size) <- zip (graphSnapshotNodes snapshot) nodeSizes]
      screenPositions = screenPoint rect <$> graphLayoutPositions layout
      selectedEdges = graphSnapshotSelectedEdge snapshot
      selectedNodes = graphSnapshotSelectedNode snapshot
  mapM_ (drawGraphEdge snapshot sizesByKey screenPositions selectedEdges) (graphSnapshotEdges snapshot)
  mapM_ (drawGraphNode screenPositions sizesByKey selectedNodes) (graphSnapshotNodes snapshot)

drawGraphEdge
  :: Canvas.Canvas renderM
  => GraphSnapshot
  -> Map GraphNodeKey Size
  -> Map GraphNodeKey Point
  -> Maybe GraphSelectedEdge
  -> GraphEdge
  -> renderM ()
drawGraphEdge snapshot sizesByKey positions selected edge =
  case (Map.lookup (graphEdgeSource edge) positions, Map.lookup (graphEdgeTarget edge) positions) of
    (Just source, Just target) ->
      if graphEdgeSource edge == graphEdgeTarget edge
        then drawSelfLoop source selectedStrength edge
        else drawStraightEdge source target selectedStrength edge
    _ -> pure ()
  where
    selectedStrength =
      case selected of
        Just GraphSelectedEdge {graphSelectedEdgeSource, graphSelectedEdgeLabel, graphSelectedEdgeStrength}
          | graphSelectedEdgeSource == graphEdgeSource edge && graphSelectedEdgeLabel == graphEdgeLabel edge -> Just graphSelectedEdgeStrength
        _ -> Nothing
    edgeOffset =
      parallelEdgeOffset snapshot edge
    drawStraightEdge source target strength currentEdge = do
      let direction = pointSub target source
          perpendicular = pointPerp (pointNormalize direction)
          offset = pointScale perpendicular edgeOffset
          shiftedSource = pointAdd source offset
          shiftedTarget = pointAdd target offset
          start = clipNode sizesByKey (graphEdgeSource currentEdge) shiftedSource shiftedTarget
          end = clipNode sizesByKey (graphEdgeTarget currentEdge) shiftedTarget shiftedSource
          labelPoint = midpoint start end
          color = edgeColor strength
          lineWidth = edgeLineWidth strength
      Canvas.strokeLine start end color lineWidth
      drawArrowhead end (pointSub end start) color lineWidth
      drawEdgeLabel currentEdge labelPoint strength
    drawSelfLoop center strength currentEdge = do
      let size = Map.findWithDefault (Size nodeMinWidth nodeHeight) (graphEdgeSource currentEdge) sizesByKey
          top = Point (pointX center) (pointY center - sizeHeight size / 2 - 34 - abs edgeOffset)
          left = Point (pointX center - sizeWidth size / 2 - 26 - abs edgeOffset / 2) (pointY center)
          right = Point (pointX center + sizeWidth size / 2 + 26 + abs edgeOffset / 2) (pointY center)
          color = edgeColor strength
          lineWidth = edgeLineWidth strength
      Canvas.strokeLine left top color lineWidth
      Canvas.strokeLine top right color lineWidth
      Canvas.strokeLine right center color lineWidth
      drawArrowhead center (pointSub center right) color lineWidth
      drawEdgeLabel currentEdge top strength

drawEdgeLabel :: Canvas.Canvas renderM => GraphEdge -> Point -> Maybe GraphSelectionStrength -> renderM ()
drawEdgeLabel edge point strength = do
  let textValue = maybe "" truncateNodeText (graphEdgeTitle edge)
  textMetrics <- Canvas.measureText textValue
  sample <- Canvas.measureText "Mg"
  let textWidth = if null textValue then 0 else Canvas.textWidth textMetrics
      labelWidth = edgeLabelPadding * 2 + edgeLabelIconSize + if null textValue then 0 else labelGap + textWidth
      labelHeight = max edgeLabelIconSize (metricHeight sample) + edgeLabelPadding * 2
      labelRect = Rect (pointX point - labelWidth / 2) (pointY point - labelHeight / 2) labelWidth labelHeight
      iconRect = Rect (x labelRect + edgeLabelPadding) (y labelRect + (height labelRect - edgeLabelIconSize) / 2) edgeLabelIconSize edgeLabelIconSize
      textPoint = Point (x iconRect + width iconRect + labelGap) (y labelRect + height labelRect / 2)
  Canvas.fillRect labelRect edgeLabelBackground
  Canvas.strokeRect labelRect (edgeColor strength) (edgeLineWidth strength)
  identicon (graphEdgeLabel edge) iconRect
  if null textValue
    then pure ()
    else Canvas.fillTextMiddle textPoint textColor textValue

drawGraphNode
  :: Canvas.Canvas renderM
  => Map GraphNodeKey Point
  -> Map GraphNodeKey Size
  -> Maybe GraphSelectedNode
  -> GraphNode
  -> renderM ()
drawGraphNode positions sizesByKey selected node =
  case (Map.lookup key positions, Map.lookup key sizesByKey) of
    (Just center, Just size) -> do
      let nodeRect = centeredRect center size
          strength = selectedStrength
      Canvas.fillRect nodeRect (nodeFill node)
      Canvas.strokeRect nodeRect (nodeStroke strength) (nodeStrokeWidth strength)
      drawGraphNodeLabel node nodeRect
    _ -> pure ()
  where
    key = graphNodeKey node
    selectedStrength =
      case selected of
        Just GraphSelectedNode {graphSelectedNodeKey, graphSelectedNodeStrength}
          | graphSelectedNodeKey == key -> Just graphSelectedNodeStrength
        _ -> Nothing

drawGraphNodeLabel :: Canvas.Canvas renderM => GraphNode -> Rect -> renderM ()
drawGraphNodeLabel node rect =
  case graphNodeTitle node of
    Just title ->
      Canvas.fillTextMiddle
        (Point (x rect + nodePadding) (y rect + height rect / 2))
        textColor
        (truncateNodeText title)
    Nothing ->
      case graphNodeUUID node of
        Just nodeUUID -> do
          let iconRect = Rect (x rect + (width rect - nodeIconSize) / 2) (y rect + (height rect - nodeIconSize) / 2) nodeIconSize nodeIconSize
          identicon nodeUUID iconRect
        Nothing -> pure ()

nodeSize :: Canvas.Canvas renderM => GraphNode -> renderM Size
nodeSize node = do
  metrics <- Canvas.measureText textValue
  sample <- Canvas.measureText "Mg"
  let titleWidth = if null textValue then 0 else Canvas.textWidth metrics
      iconWidth = if graphNodeShowsIdenticon node then nodeIconSize else 0
      contentWidth = iconWidth + titleWidth
      contentHeight = max iconWidth (metricHeight sample)
  pure (Size (max nodeMinWidth (contentWidth + nodePadding * 2)) (max nodeHeight (contentHeight + nodePadding * 2)))
  where
    textValue = maybe "" truncateNodeText (graphNodeTitle node)

graphNodeShowsIdenticon :: GraphNode -> Bool
graphNodeShowsIdenticon node =
  case (graphNodeUUID node, graphNodeTitle node) of
    (Just _, Nothing) -> True
    _ -> False

clipNode :: Map GraphNodeKey Size -> GraphNodeKey -> Point -> Point -> Point
clipNode sizesByKey key center target =
  clipToRect center half target
  where
    size = Map.findWithDefault (Size nodeMinWidth nodeHeight) key sizesByKey
    half = Point (sizeWidth size / 2) (sizeHeight size / 2)

clipToRect :: Point -> Point -> Point -> Point
clipToRect center half target
  | abs (pointX direction) < epsilon && abs (pointY direction) < epsilon =
      pointAdd center (Point (pointX half) 0)
  | otherwise =
      pointAdd center (pointScale direction scaleFactor)
  where
    direction = pointSub target center
    scaleX =
      if abs (pointX direction) < epsilon
        then maxFinite
        else pointX half / abs (pointX direction)
    scaleY =
      if abs (pointY direction) < epsilon
        then maxFinite
        else pointY half / abs (pointY direction)
    scaleFactor = min scaleX scaleY

drawArrowhead :: Canvas.Canvas renderM => Point -> Point -> String -> Double -> renderM ()
drawArrowhead tip direction color lineWidth = do
  Canvas.strokeLine tip left color lineWidth
  Canvas.strokeLine tip right color lineWidth
  where
    normal = pointNormalize direction
    perpendicular = pointPerp normal
    base = pointSub tip (pointScale normal arrowheadLength)
    left = pointAdd base (pointScale perpendicular arrowheadWidth)
    right = pointSub base (pointScale perpendicular arrowheadWidth)

parallelEdgeOffset :: GraphSnapshot -> GraphEdge -> Double
parallelEdgeOffset snapshot edge =
  case indexes of
    [] -> 0
    _ -> (fromIntegral currentIndex - (fromIntegral total - 1) / 2) * parallelEdgeSpacing
  where
    pairKey current =
      canonicalPair (graphEdgeSource current) (graphEdgeTarget current)
    samePair current =
      pairKey current == pairKey edge
    pairEdges = filter samePair (graphSnapshotEdges snapshot)
    indexes = [index | (index, current) <- zip [0 :: Int ..] pairEdges, current == edge]
    currentIndex =
      case indexes of
        index : _ -> index
        [] -> 0
    total = length pairEdges

canonicalPair :: GraphNodeKey -> GraphNodeKey -> (GraphNodeKey, GraphNodeKey)
canonicalPair a b
  | a <= b = (a, b)
  | otherwise = (b, a)

deterministicPosition :: GraphNodeKey -> Int -> Point
deterministicPosition key index =
  Point
    { pointX = (((fromIntegral low / 65535) - 0.5) * 300) + fromIntegral index * 5
    , pointY = (((fromIntegral high / 65535) - 0.5) * 200) + fromIntegral index * 5
    }
  where
    hash = hashString (show key)
    low = hash .&. 0xffff
    high = (hash `shiftR` 16) .&. 0xffff

hashString :: String -> Word32
hashString =
  foldl step 2166136261
  where
    step hash char =
      (hash `xor` fromIntegral (ord char)) * 16777619

screenPoint :: Rect -> Point -> Point
screenPoint rect point =
  Point
    { pointX = x rect + width rect / 2 + pointX point
    , pointY = y rect + height rect / 2 + pointY point
    }

centeredRect :: Point -> Size -> Rect
centeredRect point size =
  Rect
    { x = pointX point - sizeWidth size / 2
    , y = pointY point - sizeHeight size / 2
    , width = sizeWidth size
    , height = sizeHeight size
    }

pointAdd :: Point -> Point -> Point
pointAdd a b =
  Point (pointX a + pointX b) (pointY a + pointY b)

pointSub :: Point -> Point -> Point
pointSub a b =
  Point (pointX a - pointX b) (pointY a - pointY b)

pointScale :: Point -> Double -> Point
pointScale a n =
  Point (pointX a * n) (pointY a * n)

pointLength :: Point -> Double
pointLength point =
  sqrt (pointLengthSq point)

pointLengthSq :: Point -> Double
pointLengthSq point =
  pointX point * pointX point + pointY point * pointY point

pointNormalize :: Point -> Point
pointNormalize point
  | pointLength point < epsilon = Point 1 0
  | otherwise = pointScale point (1 / pointLength point)

pointPerp :: Point -> Point
pointPerp point =
  Point (negate (pointY point)) (pointX point)

midpoint :: Point -> Point -> Point
midpoint a b =
  pointScale (pointAdd a b) 0.5

clamp :: Ord a => a -> a -> a -> a
clamp low high value =
  max low (min high value)

metricHeight :: Canvas.TextMetrics -> Double
metricHeight metrics =
  Canvas.textFontBoundingBoxAscent metrics + Canvas.textFontBoundingBoxDescent metrics

truncateNodeText :: String -> String
truncateNodeText =
  truncateText 18

truncateText :: Int -> String -> String
truncateText limit string
  | length string <= limit = string
  | otherwise = take (limit - 3) string <> "..."

shortUUID :: UUID -> String
shortUUID =
  take 8 . UUID.toString

nodeFill :: GraphNode -> String
nodeFill node
  | graphNodeRoot node = rootNodeBackground
  | otherwise = nodeBackground

nodeStroke :: Maybe GraphSelectionStrength -> String
nodeStroke strength =
  case strength of
    Just GraphSelectionPrimary -> primarySelectionColor
    Just GraphSelectionSecondary -> secondarySelectionColor
    Nothing -> nodeBorder

nodeStrokeWidth :: Maybe GraphSelectionStrength -> Double
nodeStrokeWidth strength =
  case strength of
    Just _ -> 2.5
    Nothing -> 1.5

edgeColor :: Maybe GraphSelectionStrength -> String
edgeColor strength =
  case strength of
    Just GraphSelectionPrimary -> primarySelectionColor
    Just GraphSelectionSecondary -> secondarySelectionColor
    Nothing -> edgeStroke

edgeLineWidth :: Maybe GraphSelectionStrength -> Double
edgeLineWidth strength =
  case strength of
    Just _ -> 2.5
    Nothing -> 1

panelSizing :: Sizing
panelSizing =
  Sizing (Fill unbounded) (Fill unbounded)

repulsionK :: Double
repulsionK = 8000

attractionK :: Double
attractionK = 0.02

restLength :: Double
restLength = 120

damping :: Double
damping = 0.85

maxForce :: Double
maxForce = 10

gravityK :: Double
gravityK = 0.005

parallelEdgeSpacing :: Double
parallelEdgeSpacing = 22

epsilon :: Double
epsilon = 0.001

maxFinite :: Double
maxFinite = 1.0e12

nodePadding :: Double
nodePadding = 7

nodeIconSize :: Double
nodeIconSize = 24

nodeMinWidth :: Double
nodeMinWidth = 38

nodeHeight :: Double
nodeHeight = 34

edgeLabelIconSize :: Double
edgeLabelIconSize = 16

edgeLabelPadding :: Double
edgeLabelPadding = 4

edgeLabelBackground :: String
edgeLabelBackground = "#fbfbfa"

labelGap :: Double
labelGap = 5

arrowheadLength :: Double
arrowheadLength = 8

arrowheadWidth :: Double
arrowheadWidth = 4

panelBackground :: String
panelBackground = "#f7f8fb"

panelSeparator :: String
panelSeparator = "#d9dde3"

nodeBackground :: String
nodeBackground = "#ffffff"

rootNodeBackground :: String
rootNodeBackground = "#ebf0fa"

nodeBorder :: String
nodeBorder = "#777777"

edgeStroke :: String
edgeStroke = "#8f96a1"

primarySelectionColor :: String
primarySelectionColor = "#111111"

secondarySelectionColor :: String
secondarySelectionColor = "#777777"

textColor :: String
textColor = "#20242a"
