module Main
  ( main
  ) where

import qualified Data.Map.Strict as Map
import qualified Data.UUID.Types as UUID
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.MapGraph (MapGraph)
import Puri.Widgets (LineEditSelection (..))
import Test.QuickCheck

main :: IO ()
main = do
  run "setEdge" (propToolTracksValue genSetEdge)
  run "deleteEdge" (propToolTracksValue genDeleteEdge)
  run "editString" propEditStringWritesAndFocuses
  run "blurString" propBlurStringOnlyClearsMatchingPath
  run "graphContext" propGraphContextUsesLibraries
  where
    run name prop = do
      result <- quickCheckWithResult stdArgs {maxSuccess = 1000} prop
      case result of
        Success {} -> pure ()
        _ -> fail (name <> ": law failed")

-- Every tool must keep focus on the same value: write a sentinel at the
-- focused path's target, apply the tool, and surviving focus must still
-- address the sentinel (or have been dropped).
propToolTracksValue :: (MapGraph -> Gen (Editor -> Editor)) -> Property
propToolTracksValue genTool =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      tool <- genTool graph
      pure (graph, path, tool)
    check (graph, path, tool) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        case writeAt graph rootId path sentinel of
          Nothing -> counterexample "generated path did not resolve" False
          Just instrumented ->
            let edited = tool Editor {editorDocument = Document rootId instrumented, editorFocus = Just (Focus path restingState)}
             in case editorFocus edited of
                  Nothing -> property True
                  Just (Focus path' _) ->
                    counterexample
                      ("survived: " <> show path')
                      (readAt (documentGraph (editorDocument edited)) rootId path' == Just sentinel)

-- editString must write the string at the path and leave focus exactly
-- as the widget asked.
propEditStringWritesAndFocuses :: Property
propEditStringWritesAndFocuses =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      string <- elements ["x", "yz", "hello"]
      selection <- genLineEditSelection
      pure (graph, path, string, selection)
    check (graph, path, string, selection) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let edited = editString path string selection Editor {editorDocument = Document rootId graph, editorFocus = Just (Focus path restingState)}
         in (readAt (documentGraph (editorDocument edited)) rootId path === Just (VString string))
              .&&. (editorFocus edited === Just (Focus path selection))

propBlurStringOnlyClearsMatchingPath :: Property
propBlurStringOnlyClearsMatchingPath =
  forAllBlind genCase check
  where
    genCase = do
      graph <- genGraph
      path <- genPath graph
      pure (graph, path)
    check (graph, path) =
      counterexample ("graph: " <> show graph <> "\npath: " <> show path) $
        let editor = Editor {editorDocument = Document rootId graph, editorFocus = Just (Focus path restingState)}
            otherPath = path <> [head labelPool]
         in (editorFocus (blurString path editor) === Nothing)
              .&&. (editorFocus (blurString otherPath editor) === Just (Focus path restingState))

propGraphContextUsesLibraries :: Property
propGraphContextUsesLibraries =
  conjoin
    [ resolvePath libraryContext [toLibraryLabel, valueLabel] === Just (VString "library")
    , resolvePath documentWinsContext [toLibraryLabel, valueLabel] === Just (VString "document")
    , resolvePath documentWinsContext [toLibraryLabel, libraryOnlyLabel] === Nothing
    ]
  where
    root = UUID.fromWords 900 0 0 1
    libraryNode = UUID.fromWords 901 0 0 1
    toLibraryLabel = UUID.fromWords 902 0 0 1
    valueLabel = UUID.fromWords 903 0 0 1
    libraryOnlyLabel = UUID.fromWords 904 0 0 1
    rootGraph =
      Map.fromList
        [ (root, Map.fromList [(toLibraryLabel, VRef libraryNode)])
        ]
    libraryGraph =
      Map.fromList
        [ ( libraryNode
          , Map.fromList
              [ (valueLabel, VString "library")
              , (libraryOnlyLabel, VString "library only")
              ]
          )
        ]
    documentOverrideGraph =
      Map.insert libraryNode (Map.fromList [(valueLabel, VString "document")]) rootGraph
    libraryContext =
      documentContext (Document root rootGraph) [libraryGraph]
    documentWinsContext =
      documentContext (Document root documentOverrideGraph) [libraryGraph]

genLineEditSelection :: Gen LineEditSelection
genLineEditSelection =
  LineEditSelection <$> chooseInt (0, 8) <*> chooseInt (0, 8) <*> arbitrary

restingState :: LineEditSelection
restingState = LineEditSelection 0 0 False

sentinel :: Value
sentinel = VString "##sentinel##"

genSetEdge :: MapGraph -> Gen (Editor -> Editor)
genSetEdge _graph =
  setEdge <$> genAnyEdge <*> genValue

genDeleteEdge :: MapGraph -> Gen (Editor -> Editor)
genDeleteEdge _graph =
  deleteEdge <$> genAnyEdge

genAnyEdge :: Gen Edge
genAnyEdge =
  Edge <$> elements nodePool <*> elements labelPool

-- Independent resolver (separate from the tools' own path walk) so the
-- properties check two implementations against each other.
readAt :: MapGraph -> UUID -> [UUID] -> Maybe Value
readAt graph node path =
  case path of
    [] -> Nothing
    label : rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup label edges
      case (rest, value) of
        ([], _) -> Just value
        (_, VRef target) -> readAt graph target rest
        _ -> Nothing

writeAt :: MapGraph -> UUID -> [UUID] -> Value -> Maybe MapGraph
writeAt graph node path newValue =
  case path of
    [] -> Nothing
    label : rest -> do
      edges <- Map.lookup node graph
      value <- Map.lookup label edges
      case (rest, value) of
        ([], VString _) -> Just (Map.insert node (Map.insert label newValue edges) graph)
        (_, VRef target) -> writeAt graph target rest newValue
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
      -- a string edge and the path walk always terminates.
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

-- Random walk to a string spot, bounded against cycles, retried until
-- it lands.
genPath :: MapGraph -> Gen [UUID]
genPath graph =
  walkNode rootId (8 :: Int) `suchThatMap` id
  where
    walkNode node fuel =
      case Map.lookup node graph of
        Nothing -> pure Nothing
        Just edges -> do
          (label, value) <- elements (Map.toList edges)
          case value of
            VString _ -> pure (Just [label])
            VRef target | fuel > 0 -> fmap (label :) <$> walkNode target (fuel - 1)
            _ -> pure Nothing
