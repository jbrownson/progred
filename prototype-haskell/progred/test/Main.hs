module Main
  ( main
  ) where

import qualified Data.Map.Strict as Map
import qualified Data.UUID.Types as UUID
import Progred.Graph
import Progred.MapGraph
import Progred.Render.Raw
import Puri.Widgets.LineEdit (EditView (..))
import Test.QuickCheck

main :: IO ()
main = do
  result <- quickCheckWithResult stdArgs {maxSuccess = 1000} propTransportTracksValue
  case result of
    Success {} -> pure ()
    _ -> fail "focus transport law failed"

-- Transport must keep the chain on the same value: write a sentinel at
-- the chain's target, apply the delta, and the transported chain must
-- still address the sentinel (or be dropped).
propTransportTracksValue :: Property
propTransportTracksValue =
  forAll genCase check
  where
    check (graph, focus, delta) =
      case writeAt graph rootId focus sentinel of
        Nothing -> counterexample "generated chain did not resolve" False
        Just instrumented ->
          let transported = transportFocus instrumented rootId delta focus
              after = applyDelta delta instrumented
           in case transported of
                Nothing -> property True
                Just focus' ->
                  counterexample
                    ("focus:      " <> show focus <> "\ntransported: " <> show focus' <> "\nresolved:   " <> show (readAt after rootId focus'))
                    (readAt after rootId focus' == Just sentinel)

sentinel :: Value
sentinel = VString "##sentinel##"

-- Independent resolver (separate from transportFocus) so the property
-- checks two implementations against each other.
readAt :: MapGraph -> UUID -> Focus -> Maybe Value
readAt graph node focus =
  case focus of
    FocusText _ -> Nothing
    FocusEdge label rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup label edges
      case (value, rest) of
        (_, FocusText _) -> Just value
        (VRef target, _) -> readAt graph target rest
        _ -> Nothing

writeAt :: MapGraph -> UUID -> Focus -> Value -> Maybe MapGraph
writeAt graph node focus newValue =
  case focus of
    FocusText _ -> Nothing
    FocusEdge label rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup label edges
      case (value, rest) of
        (VString _, FocusText _) -> Just (Map.insert node (Map.insert label newValue edges) graph)
        (VRef target, _) -> writeAt graph target rest newValue
        _ -> Nothing

nodePool :: [UUID]
nodePool = uuidsFrom 1 8

labelPool :: [UUID]
labelPool = uuidsFrom 100 8

rootId :: UUID
rootId = head nodePool

uuidsFrom :: Int -> Int -> [UUID]
uuidsFrom start count =
  [UUID.fromWords (fromIntegral seed) 0 0 1 | seed <- [start .. start + count - 1]]

genCase :: Gen (MapGraph, Focus, MapGraphDelta)
genCase = do
  graph <- genGraph
  focus <- genFocus graph
  delta <- genDelta graph
  pure (graph, focus, delta)

genGraph :: Gen MapGraph
genGraph = do
  nodes <- sublistOf nodePool `suchThat` elem rootId
  Map.fromList <$> traverse genNode nodes
  where
    genNode node = do
      labels <- sublistOf labelPool `suchThat` (not . null)
      edges <- traverse genEdge labels
      anchor <- genEdge (head labelPool)
      -- fromList is right-biased; the anchor must win so every node keeps
      -- a string edge and the focus walk always terminates.
      pure (node, Map.fromList (edges <> [forceString anchor]))
    genEdge label = (,) label <$> genValue
    forceString (label, _) = (label, VString "anchor")

genValue :: Gen Value
genValue =
  frequency
    [ (3, VString <$> elements ["a", "bb", "ccc"])
    , (1, VInt <$> arbitrary)
    , (1, VBool <$> arbitrary)
    , (2, VRef <$> elements nodePool)
    ]

-- Random walk mirroring the projection's structure, bounded against
-- cycles, retried until it ends at a string.
genFocus :: MapGraph -> Gen Focus
genFocus graph =
  walkNode rootId (8 :: Int) `suchThatMap` id
  where
    walkNode node fuel =
      case Map.lookup node graph of
        Nothing -> pure Nothing
        Just edges -> do
          (label, value) <- elements (Map.toList edges)
          fmap (FocusEdge label) <$> walkValue value fuel
    walkValue value fuel =
      case value of
        VString _ -> pure (Just (FocusText (EditView 0 0 False)))
        VRef target | fuel > 0 -> walkNode target (fuel - 1)
        _ -> pure Nothing

genDelta :: MapGraph -> Gen MapGraphDelta
genDelta graph = do
  node <- elements (Map.keys graph)
  nodeDelta <-
    oneof
      [ pure (NodeDelta True Map.empty)
      , edgeDelta
      ]
  pure (MapGraphDelta (Map.singleton node nodeDelta))
  where
    edgeDelta = do
      label <- elements labelPool
      change <- oneof [pure Nothing, Just <$> genValue]
      pure (NodeDelta False (Map.singleton label change))
