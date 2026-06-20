module Progred.Render.Graph
  ( GraphEdge (..)
  , GraphDrag (..)
  , GraphEdgeLabelHit (..)
  , GraphLayout (..)
  , GraphNode (..)
  , GraphNodeKey (..)
  , GraphPan (..)
  , GraphPanelActions (..)
  , GraphSelection (..)
  , GraphSelectionStrength (..)
  , GraphSnapshot (..)
  , secondarySelectionUUID
  , GraphSelectedEdge (..)
  , GraphSelectedNode (..)
  , GraphViewport (..)
  , applyGraphWheel
  , graphClickMoveThreshold
  , graphPointerExceededClickThreshold
  , graphEdgeLabelHitAreas
  , emptyGraphLayout
  , emptyGraphViewport
  , graphLayoutNodeCount
  , graphLayoutPosition
  , graphPanel
  , graphSnapshot
  , moveGraphNode
  , moveGraphPan
  , nodeSize
  , stepGraphLayout
  ) where

import Control.Applicative ((<|>))
import Control.Monad (guard)
import Data.Maybe (catMaybes)
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

data GraphSelection
  = GraphSelectNode GraphNodeKey
  | GraphSelectEdge GraphNodeKey UUID
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

data GraphViewport = GraphViewport
  { graphViewportPan :: Point
  , graphViewportZoom :: Double
  }
  deriving (Eq, Show)

data GraphDrag = GraphDrag
  { graphDragNode :: GraphNodeKey
  , graphDragOffset :: Point
  , graphDragPosition :: Point
  }
  deriving (Eq, Show)

data GraphPan = GraphPan
  { graphPanLastScreen :: Point
  }
  deriving (Eq, Show)

data GraphEdgeLabelHit = GraphEdgeLabelHit
  { graphEdgeLabelHitEdge :: GraphEdge
  , graphEdgeLabelHitRect :: Rect
  }
  deriving (Eq, Show)

data GraphPanelActions actionM = GraphPanelActions
  { graphPanelDrag :: Maybe GraphDrag
  , graphPanelPan :: Maybe GraphPan
  , graphPanelEdgePress :: Maybe GraphEdge
  , graphPanelViewport :: GraphViewport
  , graphPanelPointerOrigin :: Maybe Point
  , graphPanelPointerMoved :: Bool
  , graphPanelDragStart :: GraphDrag -> actionM ()
  , graphPanelDragMove :: Point -> actionM ()
  , graphPanelDragEnd :: actionM ()
  , graphPanelPanStart :: Point -> actionM ()
  , graphPanelPanMove :: Point -> actionM ()
  , graphPanelPanEnd :: actionM ()
  , graphPanelEdgePressStart :: GraphEdge -> actionM ()
  , graphPanelEdgePressEnd :: actionM ()
  , graphPanelSetViewport :: GraphViewport -> actionM ()
  , graphPanelInteractionStart :: Point -> actionM ()
  , graphPanelInteractionMove :: Point -> actionM ()
  , graphPanelSetSelection :: Maybe GraphSelection -> actionM ()
  }

emptyGraphViewport :: GraphViewport
emptyGraphViewport =
  GraphViewport
    { graphViewportPan = Point 0 0
    , graphViewportZoom = 1
    }

emptyGraphLayout :: GraphLayout
emptyGraphLayout =
  GraphLayout
    { graphLayoutPositions = Map.empty
    , graphLayoutVelocities = Map.empty
    }

graphLayoutNodeCount :: GraphLayout -> Int
graphLayoutNodeCount =
  Map.size . graphLayoutPositions

graphLayoutPosition :: GraphNodeKey -> GraphLayout -> Maybe Point
graphLayoutPosition key =
  Map.lookup key . graphLayoutPositions

moveGraphNode :: GraphNodeKey -> Point -> GraphLayout -> GraphLayout
moveGraphNode key position layout =
  layout
    { graphLayoutPositions = Map.insert key position (graphLayoutPositions layout)
    , graphLayoutVelocities = Map.insert key (Point 0 0) (graphLayoutVelocities layout)
    }

graphSnapshot :: Editor -> Maybe GraphSelection -> GraphSnapshot
graphSnapshot editor graphSelection =
  documentGraphSnapshot (editorDocument editor) (editorFocus editor) graphSelection

documentGraphSnapshot :: Document -> Maybe Focus -> Maybe GraphSelection -> GraphSnapshot
documentGraphSnapshot document focus graphSelection =
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
      case graphSelection of
        Just (GraphSelectNode key) -> Just (GraphSelectedNode key GraphSelectionPrimary)
        Nothing ->
          focusedNodeKey document context focus <&> \key ->
            GraphSelectedNode key GraphSelectionSecondary
        Just (GraphSelectEdge _ _) -> Nothing
    selectedEdge =
      case graphSelection of
        Just (GraphSelectEdge source label) ->
          Just (GraphSelectedEdge source label GraphSelectionPrimary)
        Nothing ->
          focusedEdge context focus <&> \Edge {edgeSource, edgeLabel} ->
            GraphSelectedEdge (GraphUUID edgeSource) edgeLabel GraphSelectionSecondary
        Just (GraphSelectNode _) -> Nothing

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

secondarySelectionUUID :: Editor -> Maybe GraphSelection -> Maybe UUID
secondarySelectionUUID editor graphSelection =
  case graphSelection of
    Just (GraphSelectNode (GraphUUID uuid)) -> Just uuid
    Just (GraphSelectNode _) -> Nothing
    Just (GraphSelectEdge _ _) -> Nothing
    Nothing -> focusedUUID editor
  where
    focusedUUID current = do
      focus <- editorFocus current
      let document = editorDocument current
          context = documentContext document []
      key <- focusedNodeKey document context (Just focus)
      case key of
        GraphUUID uuid -> Just uuid
        _ -> Nothing

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

finishGraphInteraction :: Monad actionM => GraphPanelActions actionM -> Maybe GraphSelection -> actionM ()
finishGraphInteraction actions selection
  | graphPanelPointerMoved actions = pure ()
  | otherwise = graphPanelSetSelection actions selection

graphClickMoveThreshold :: Double
graphClickMoveThreshold = 2

graphPointerExceededClickThreshold :: Point -> Point -> Bool
graphPointerExceededClickThreshold origin pointer =
  let Point {pointX = x0, pointY = y0} = origin
      Point {pointX = x1, pointY = y1} = pointer
      threshold = graphClickMoveThreshold
      dx = x1 - x0
      dy = y1 - y0
   in dx * dx + dy * dy > threshold * threshold

graphPanel :: (Canvas.Canvas renderM, Monad actionM) => GraphSnapshot -> GraphViewport -> GraphLayout -> GraphPanelActions actionM -> Halay renderM renderM (Handler actionM)
graphPanel snapshot viewport layout actions =
  leafWithSizing panelSizing (pure (Size 320 240)) draw
  where
    draw rect = do
      let syncedLayout = syncGraphLayout snapshot layout
      drawGraphPanel snapshot viewport syncedLayout rect
      handler <- graphPanelHandler snapshot viewport syncedLayout actions rect
      pure (handler <> graphPanelWheelHandler actions rect)

graphPanelHandler :: (Canvas.Canvas renderM, Monad actionM) => GraphSnapshot -> GraphViewport -> GraphLayout -> GraphPanelActions actionM -> Rect -> renderM (Handler actionM)
graphPanelHandler snapshot viewport layout actions rect = do
  nodeSizes <- traverse nodeSize (graphSnapshotNodes snapshot)
  edgeHits <- graphEdgeLabelHitAreas snapshot viewport layout rect nodeSizes
  let zoom = graphViewportZoom viewport
      sizesByKey = Map.fromList [(graphNodeKey node, size) | (node, size) <- zip (graphSnapshotNodes snapshot) nodeSizes]
      screenSizesByKey = fmap (scaleScreenSize zoom) sizesByKey
      graphPositions = graphLayoutPositions layout
      screenPositions = screenPoint viewport rect <$> graphPositions
  pure $
    onPointerCapture $ \event ->
      case event of
        PointerDown {pointerX, pointerY}
          | rectContains rect pointerX pointerY ->
              let pointer = Point pointerX pointerY
               in Just $ do
                    graphPanelInteractionStart actions pointer
                    case hitGraphEdge edgeHits pointer of
                      Just edge -> graphPanelEdgePressStart actions edge
                      Nothing ->
                        case
                          hitGraphNode
                            snapshot
                            screenSizesByKey
                            graphPositions
                            screenPositions
                            pointer
                            (pointerGraphPoint viewport rect pointerX pointerY)
                          of
                            Just drag -> graphPanelDragStart actions drag
                            Nothing -> graphPanelPanStart actions pointer
        PointerMove {pointerX, pointerY}
          | graphPanelDrag actions /= Nothing
              || graphPanelPan actions /= Nothing
              || graphPanelEdgePress actions /= Nothing ->
              let pointer = Point pointerX pointerY
               in Just $ do
                    graphPanelInteractionMove actions pointer
                    case graphPanelDrag actions of
                      Just activeDrag ->
                        graphPanelDragMove actions (dragGraphPoint viewport rect activeDrag pointerX pointerY)
                      Nothing
                        | graphPanelPan actions /= Nothing ->
                            graphPanelPanMove actions pointer
                      _ -> pure ()
        PointerUp {}
          | Just edge <- graphPanelEdgePress actions ->
              Just $ do
                finishGraphInteraction
                  actions
                  (Just (GraphSelectEdge (graphEdgeSource edge) (graphEdgeLabel edge)))
                graphPanelEdgePressEnd actions
          | Just drag <- graphPanelDrag actions ->
              Just $ do
                finishGraphInteraction actions (Just (GraphSelectNode (graphDragNode drag)))
                graphPanelDragEnd actions
          | Just _ <- graphPanelPan actions ->
              Just $ do
                finishGraphInteraction actions Nothing
                graphPanelPanEnd actions
        _ -> Nothing

graphPanelWheelHandler :: GraphPanelActions actionM -> Rect -> Handler actionM
graphPanelWheelHandler actions rect =
  onWheel $ \event@Wheel {wheelX, wheelY} ->
    if rectContains rect wheelX wheelY
      then
        Just
          ( graphPanelSetViewport actions
              (applyGraphWheel (graphPanelViewport actions) rect event)
          )
      else Nothing

graphEdgeLabelHitAreas
  :: Canvas.Canvas renderM
  => GraphSnapshot
  -> GraphViewport
  -> GraphLayout
  -> Rect
  -> [Size]
  -> renderM [GraphEdgeLabelHit]
graphEdgeLabelHitAreas snapshot viewport layout rect nodeSizes = do
  let sizesByKey = Map.fromList [(graphNodeKey node, size) | (node, size) <- zip (graphSnapshotNodes snapshot) nodeSizes]
      graphPositions = graphLayoutPositions layout
      zoom = graphViewportZoom viewport
  hits <-
    traverse
      ( \edge -> do
          size <- edgeLabelSize edge
          case graphEdgeGeometry snapshot sizesByKey graphPositions edge of
            Nothing -> pure Nothing
            Just geometry ->
              pure $
                Just
                  GraphEdgeLabelHit
                    { graphEdgeLabelHitEdge = edge
                    , graphEdgeLabelHitRect =
                        centeredRect
                          (screenPoint viewport rect (graphEdgeGeometryLabelPoint geometry))
                          (scaleScreenSize zoom size)
                    }
      )
      (graphSnapshotEdges snapshot)
  pure (catMaybes hits)

hitGraphEdge :: [GraphEdgeLabelHit] -> Point -> Maybe GraphEdge
hitGraphEdge hits pointer =
  foldr hit Nothing hits
  where
    hit GraphEdgeLabelHit {graphEdgeLabelHitEdge, graphEdgeLabelHitRect} found =
      found <|> do
        guard (rectContains graphEdgeLabelHitRect (pointX pointer) (pointY pointer))
        pure graphEdgeLabelHitEdge

data GraphEdgeCurve
  = GraphEdgeLine Point Point
  | GraphEdgeQuadratic Point Point Point
  | GraphEdgeCubic Point Point Point Point
  deriving (Eq, Show)

data GraphEdgeGeometry = GraphEdgeGeometry
  { graphEdgeGeometryCurve :: GraphEdgeCurve
  , graphEdgeGeometryLabelPoint :: Point
  }
  deriving (Eq, Show)

graphEdgeGeometry
  :: GraphSnapshot
  -> Map GraphNodeKey Size
  -> Map GraphNodeKey Point
  -> GraphEdge
  -> Maybe GraphEdgeGeometry
graphEdgeGeometry snapshot sizesByKey positions edge =
  case (Map.lookup (graphEdgeSource edge) positions, Map.lookup (graphEdgeTarget edge) positions) of
    (Just source, Just target)
      | graphEdgeSource edge == graphEdgeTarget edge ->
          Just (selfLoopGeometry snapshot sizesByKey edge source)
      | otherwise ->
          Just (nodeEdgeGeometry snapshot sizesByKey edge source target)
    _ -> Nothing

nodeEdgeGeometry
  :: GraphSnapshot
  -> Map GraphNodeKey Size
  -> GraphEdge
  -> Point
  -> Point
  -> GraphEdgeGeometry
nodeEdgeGeometry snapshot sizesByKey edge source target =
  let edgeOffset = parallelEdgeOffset snapshot edge
   in if abs edgeOffset < epsilon
        then
          let start = clipEdgeEndpoint sizesByKey (graphEdgeSource edge) source source target
              end = clipEdgeEndpoint sizesByKey (graphEdgeTarget edge) target target source
           in
            GraphEdgeGeometry
              { graphEdgeGeometryCurve = GraphEdgeLine start end
              , graphEdgeGeometryLabelPoint = midpoint start end
              }
        else
          let canonicalDirection =
                if graphEdgeSource edge <= graphEdgeTarget edge
                  then pointNormalize (pointSub target source)
                  else pointNormalize (pointSub source target)
              control = pointAdd (midpoint source target) (pointScale (pointPerp canonicalDirection) edgeOffset)
              start = clipToRect source (nodeHalfSize sizesByKey (graphEdgeSource edge)) control
              end = clipToRect target (nodeHalfSize sizesByKey (graphEdgeTarget edge)) control
           in
            GraphEdgeGeometry
              { graphEdgeGeometryCurve = GraphEdgeQuadratic start control end
              , graphEdgeGeometryLabelPoint = quadraticPoint start control end 0.5
              }

selfLoopGeometry :: GraphSnapshot -> Map GraphNodeKey Size -> GraphEdge -> Point -> GraphEdgeGeometry
selfLoopGeometry snapshot sizesByKey edge center =
  let edgeOffset = parallelEdgeOffset snapshot edge
      size = nodeSizeByKey sizesByKey (graphEdgeSource edge)
      half = nodeHalfSize sizesByKey (graphEdgeSource edge)
      loopHeight = sizeHeight size * 2 + 24 + abs edgeOffset
      loopWidth = sizeWidth size * 0.75 + 22 + abs edgeOffset / 2
      cp1 = pointAdd center (Point (negate loopWidth) (negate loopHeight))
      cp2 = pointAdd center (Point loopWidth (negate loopHeight))
      start = clipToRect center half cp1
      end = clipToRect center half cp2
   in
    GraphEdgeGeometry
      { graphEdgeGeometryCurve = GraphEdgeCubic start cp1 cp2 end
      , graphEdgeGeometryLabelPoint = cubicPoint start cp1 cp2 end 0.5
      }

graphEdgeArrowDirection :: GraphEdgeGeometry -> Point
graphEdgeArrowDirection geometry =
  case graphEdgeGeometryCurve geometry of
    GraphEdgeLine start end -> pointSub end start
    GraphEdgeQuadratic _ control end -> pointSub end control
    GraphEdgeCubic _ _ cp2 end -> pointSub end cp2

graphEdgeArrowTip :: GraphEdgeGeometry -> Point
graphEdgeArrowTip geometry =
  case graphEdgeGeometryCurve geometry of
    GraphEdgeLine _ end -> end
    GraphEdgeQuadratic _ _ end -> end
    GraphEdgeCubic _ _ _ end -> end

edgeLabelSize :: Canvas.Canvas renderM => GraphEdge -> renderM Size
edgeLabelSize edge = do
  let textValue = maybe "" truncateNodeText (graphEdgeTitle edge)
  textMetrics <- Canvas.measureText textValue
  sample <- Canvas.measureText "Mg"
  let textWidth = if null textValue then 0 else Canvas.textWidth textMetrics
      labelWidth = edgeLabelPadding * 2 + edgeLabelIconSize + if null textValue then 0 else labelGap + textWidth
      labelHeight = max edgeLabelIconSize (metricHeight sample) + edgeLabelPadding * 2
  pure (Size labelWidth labelHeight)

hitGraphNode
  :: GraphSnapshot
  -> Map GraphNodeKey Size
  -> Map GraphNodeKey Point
  -> Map GraphNodeKey Point
  -> Point
  -> Point
  -> Maybe GraphDrag
hitGraphNode snapshot sizesByKey graphPositions screenPositions pointer graphPointer =
  foldr hit Nothing (graphSnapshotNodes snapshot)
  where
    hit node found =
      found <|> do
        let key = graphNodeKey node
        center <- Map.lookup key screenPositions
        size <- Map.lookup key sizesByKey
        if rectContains (centeredRect center size) (pointX pointer) (pointY pointer)
          then do
            graphPosition <- Map.lookup key graphPositions
            Just (GraphDrag key (pointSub graphPosition graphPointer) graphPosition)
          else Nothing

moveGraphPan :: Point -> GraphViewport -> GraphPan -> (GraphViewport, GraphPan)
moveGraphPan current viewport pan =
  let delta = pointSub current (graphPanLastScreen pan)
   in
    ( viewport {graphViewportPan = pointAdd (graphViewportPan viewport) delta}
    , pan {graphPanLastScreen = current}
    )

applyGraphWheel :: GraphViewport -> Rect -> WheelEvent -> GraphViewport
applyGraphWheel viewport rect event =
  if wheelZooms event
    then zoomGraphWheel viewport rect event
    else panGraphWheel viewport event

wheelZooms :: WheelEvent -> Bool
wheelZooms Wheel {wheelModifiers, wheelDeltaMode} =
  hasModifier wheelModifiers || wheelDeltaMode /= 0

zoomGraphWheel :: GraphViewport -> Rect -> WheelEvent -> GraphViewport
zoomGraphWheel viewport rect event@Wheel {wheelX, wheelY, wheelDeltaY} =
  let graphPosition = pointerGraphPoint viewport rect wheelX wheelY
      newZoom =
        clamp graphMinZoom graphMaxZoom
          (graphViewportZoom viewport * exp (negate wheelDeltaY * wheelZoomFactor event))
      center = graphPanelCenter rect
      newPan =
        pointSub
          (Point wheelX wheelY)
          (pointAdd center (pointScale graphPosition newZoom))
   in
    GraphViewport
      { graphViewportPan = newPan
      , graphViewportZoom = newZoom
      }

panGraphWheel :: GraphViewport -> WheelEvent -> GraphViewport
panGraphWheel viewport Wheel {wheelDeltaX, wheelDeltaY} =
  viewport
    { graphViewportPan =
        pointAdd
          (graphViewportPan viewport)
          (Point (negate wheelDeltaX * graphWheelPanFactor) (negate wheelDeltaY * graphWheelPanFactor))
    }

wheelZoomFactor :: WheelEvent -> Double
wheelZoomFactor Wheel {wheelModifiers} =
  if keyCtrl wheelModifiers || keyMeta wheelModifiers
    then graphTrackpadZoomFactor
    else graphMouseWheelZoomFactor

dragGraphPoint :: GraphViewport -> Rect -> GraphDrag -> Double -> Double -> Point
dragGraphPoint viewport rect drag pointerX pointerY =
  pointAdd (pointerGraphPoint viewport rect pointerX pointerY) (graphDragOffset drag)

pointerGraphPoint :: GraphViewport -> Rect -> Double -> Double -> Point
pointerGraphPoint viewport rect pointerX pointerY =
  let center = graphPanelCenter rect
      screen = Point pointerX pointerY
   in pointScale (pointSub (pointSub screen center) (graphViewportPan viewport)) (1 / graphViewportZoom viewport)

drawGraphPanel :: Canvas.Canvas renderM => GraphSnapshot -> GraphViewport -> GraphLayout -> Rect -> renderM ()
drawGraphPanel snapshot viewport layout rect = do
  Canvas.fillRect rect panelBackground
  Canvas.strokeLine (Point (x rect) (y rect)) (Point (x rect) (y rect + height rect)) panelSeparator 1
  nodeSizes <- traverse nodeSize (graphSnapshotNodes snapshot)
  let sizesByKey = Map.fromList [(graphNodeKey node, size) | (node, size) <- zip (graphSnapshotNodes snapshot) nodeSizes]
      graphPositions = graphLayoutPositions layout
      transformOrigin = pointAdd (graphPanelCenter rect) (graphViewportPan viewport)
      selectedEdges = graphSnapshotSelectedEdge snapshot
      selectedNodes = graphSnapshotSelectedNode snapshot
  Canvas.withClip rect $
    Canvas.withGraphTransform transformOrigin (graphViewportZoom viewport) $ do
      mapM_ (drawGraphEdge snapshot sizesByKey graphPositions selectedEdges) (graphSnapshotEdges snapshot)
      mapM_ (drawGraphNode graphPositions sizesByKey selectedNodes) (graphSnapshotNodes snapshot)

drawGraphEdge
  :: Canvas.Canvas renderM
  => GraphSnapshot
  -> Map GraphNodeKey Size
  -> Map GraphNodeKey Point
  -> Maybe GraphSelectedEdge
  -> GraphEdge
  -> renderM ()
drawGraphEdge snapshot sizesByKey positions selected edge =
  case graphEdgeGeometry snapshot sizesByKey positions edge of
    Nothing -> pure ()
    Just geometry -> do
      let strength =
            case selected of
              Just GraphSelectedEdge {graphSelectedEdgeSource, graphSelectedEdgeLabel, graphSelectedEdgeStrength}
                | graphSelectedEdgeSource == graphEdgeSource edge && graphSelectedEdgeLabel == graphEdgeLabel edge ->
                    Just graphSelectedEdgeStrength
              _ -> Nothing
          color = edgeColor strength
          lineWidth = edgeLineWidth strength
      drawGraphEdgeCurve geometry color lineWidth
      drawArrowhead (graphEdgeArrowTip geometry) (graphEdgeArrowDirection geometry) color lineWidth
      drawEdgeLabel edge (graphEdgeGeometryLabelPoint geometry) strength

drawGraphEdgeCurve :: Canvas.Canvas renderM => GraphEdgeGeometry -> String -> Double -> renderM ()
drawGraphEdgeCurve geometry color lineWidth =
  case graphEdgeGeometryCurve geometry of
    GraphEdgeLine start end -> Canvas.strokeLine start end color lineWidth
    GraphEdgeQuadratic start control end -> drawQuadratic start control end color lineWidth
    GraphEdgeCubic start cp1 cp2 end -> drawCubic start cp1 cp2 end color lineWidth

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
  pure (Size (contentWidth + nodePadding * 2) (max nodeHeight (contentHeight + nodePadding * 2)))
  where
    textValue = maybe "" truncateNodeText (graphNodeTitle node)

graphNodeShowsIdenticon :: GraphNode -> Bool
graphNodeShowsIdenticon node =
  case (graphNodeUUID node, graphNodeTitle node) of
    (Just _, Nothing) -> True
    _ -> False

nodeSizeByKey :: Map GraphNodeKey Size -> GraphNodeKey -> Size
nodeSizeByKey sizesByKey key =
  Map.findWithDefault (Size (nodeIconSize + nodePadding * 2) nodeHeight) key sizesByKey

nodeHalfSize :: Map GraphNodeKey Size -> GraphNodeKey -> Point
nodeHalfSize sizesByKey key =
  Point (sizeWidth size / 2) (sizeHeight size / 2)
  where
    size = nodeSizeByKey sizesByKey key

clipEdgeEndpoint :: Map GraphNodeKey Size -> GraphNodeKey -> Point -> Point -> Point -> Point
clipEdgeEndpoint sizesByKey key nodeCenter linePoint lineTarget =
  clipLineToRect nodeCenter (nodeHalfSize sizesByKey key) linePoint lineTarget

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

clipLineToRect :: Point -> Point -> Point -> Point -> Point
clipLineToRect center half from target =
  case candidates of
    candidate : _ -> pointAt candidate
    [] -> clipToRect center half target
  where
    direction = pointSub target from
    pointAt t =
      pointAdd from (pointScale direction t)
    candidates =
      sortOn
        id
        (verticalCandidates <> horizontalCandidates)
    verticalCandidates =
      sideCandidates
        (pointX direction)
        (pointX from)
        (pointX center)
        (pointX half)
        (pointY from)
        (pointY direction)
        (pointY center)
        (pointY half)
    horizontalCandidates =
      sideCandidates
        (pointY direction)
        (pointY from)
        (pointY center)
        (pointY half)
        (pointX from)
        (pointX direction)
        (pointX center)
        (pointX half)

sideCandidates :: Double -> Double -> Double -> Double -> Double -> Double -> Double -> Double -> [Double]
sideCandidates primaryDirection primaryFrom primaryCenter primaryHalf secondaryFrom secondaryDirection secondaryCenter secondaryHalf
  | abs primaryDirection < epsilon = []
  | otherwise =
      filter validCandidate $
        [ (primaryCenter - primaryHalf - primaryFrom) / primaryDirection
        , (primaryCenter + primaryHalf - primaryFrom) / primaryDirection
        ]
  where
    validCandidate t =
      t >= 0
        && let secondary = secondaryFrom + t * secondaryDirection
            in secondary >= secondaryCenter - secondaryHalf - epsilon
                && secondary <= secondaryCenter + secondaryHalf + epsilon

drawQuadratic :: Canvas.Canvas renderM => Point -> Point -> Point -> String -> Double -> renderM ()
drawQuadratic start control end color lineWidth =
  mapM_ drawSegment (zip points (drop 1 points))
  where
    points =
      [ quadraticPoint start control end (fromIntegral index / fromIntegral curveSegments)
      | index <- [0 .. curveSegments]
      ]
    drawSegment (from, to) =
      Canvas.strokeLine from to color lineWidth

quadraticPoint :: Point -> Point -> Point -> Double -> Point
quadraticPoint start control end t =
  Point
    { pointX =
        quadraticCoordinate
          (pointX start)
          (pointX control)
          (pointX end)
    , pointY =
        quadraticCoordinate
          (pointY start)
          (pointY control)
          (pointY end)
    }
  where
    mt = 1 - t
    quadraticCoordinate startValue controlValue endValue =
      mt * mt * startValue
        + 2 * mt * t * controlValue
        + t * t * endValue

drawCubic :: Canvas.Canvas renderM => Point -> Point -> Point -> Point -> String -> Double -> renderM ()
drawCubic start cp1 cp2 end color lineWidth =
  mapM_ drawSegment (zip points (drop 1 points))
  where
    points =
      [ cubicPoint start cp1 cp2 end (fromIntegral index / fromIntegral curveSegments)
      | index <- [0 .. curveSegments]
      ]
    drawSegment (from, to) =
      Canvas.strokeLine from to color lineWidth

cubicPoint :: Point -> Point -> Point -> Point -> Double -> Point
cubicPoint start cp1 cp2 end t =
  Point
    { pointX =
        cubicCoordinate
          (pointX start)
          (pointX cp1)
          (pointX cp2)
          (pointX end)
    , pointY =
        cubicCoordinate
          (pointY start)
          (pointY cp1)
          (pointY cp2)
          (pointY end)
    }
  where
    mt = 1 - t
    cubicCoordinate startValue cp1Value cp2Value endValue =
      mt * mt * mt * startValue
        + 3 * mt * mt * t * cp1Value
        + 3 * mt * t * t * cp2Value
        + t * t * t * endValue

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

graphPanelCenter :: Rect -> Point
graphPanelCenter rect =
  Point
    { pointX = x rect + width rect / 2
    , pointY = y rect + height rect / 2
    }

scaleScreenSize :: Double -> Size -> Size
scaleScreenSize zoom size =
  Size (sizeWidth size * zoom) (sizeHeight size * zoom)

screenPoint :: GraphViewport -> Rect -> Point -> Point
screenPoint viewport rect point =
  let center = graphPanelCenter rect
   in pointAdd center (pointAdd (graphViewportPan viewport) (pointScale point (graphViewportZoom viewport)))

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

curveSegments :: Int
curveSegments = 20

graphMinZoom :: Double
graphMinZoom = 0.1

graphMaxZoom :: Double
graphMaxZoom = 5

graphWheelPanFactor :: Double
graphWheelPanFactor = 0.85

graphMouseWheelZoomFactor :: Double
graphMouseWheelZoomFactor = 0.004

graphTrackpadZoomFactor :: Double
graphTrackpadZoomFactor = 0.012

nodePadding :: Double
nodePadding = 7

nodeIconSize :: Double
nodeIconSize = 24

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
