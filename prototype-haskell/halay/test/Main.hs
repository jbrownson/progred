module Main
  ( main
  ) where

import Control.Exception (evaluate)
import Control.Monad (forM_, unless)
import Data.Functor.Identity
import Data.List (intercalate, mapAccumL)
import Data.Maybe (fromMaybe)
import Halay
import System.Directory (getTemporaryDirectory)
import System.Environment (lookupEnv)
import System.Exit (ExitCode (..))
import System.Process (readProcessWithExitCode)
import System.Timeout (timeout)
import Test.QuickCheck
  ( Arbitrary (..)
  , Gen
  , Args (..)
  , NonNegative (..)
  , Property
  , chooseInt
  , counterexample
  , elements
  , frequency
  , ioProperty
  , maxSuccess
  , quickCheckWithResult
  , Result (..)
  , shrinkList
  , stdArgs
  , vectorOf
  )
import Text.Read (readMaybe)

newtype Placements = Placements [(String, Rect)]
  deriving (Eq, Show)

data ClayRect = ClayRect
  { clayCase :: String
  , clayId :: String
  , clayRect :: Rect
  }
  deriving (Eq, Show)

instance Semigroup Placements where
  Placements left <> Placements right =
    Placements (left <> right)

instance Monoid Placements where
  mempty = Placements []

main :: IO ()
main = do
  flatArgs <- fuzzArgs stdArgs "HALAY_QUICKCHECK" 100
  textArgs <- fuzzArgs stdArgs "HALAY_TEXT_QUICKCHECK" 100
  treeArgs <- fuzzArgs treeQuickCheckArgs "HALAY_TREE_QUICKCHECK" 20
  oracle <- compileClayOracle
  clayRects <- runClayOracle oracle
  forM_ conformanceCases (assertClayCase clayRects)
  assertMeasuredSize "raw_text_ignores_aspect_ratio" (Size 10 1) rawTextIgnoresAspectRatio
  assertMeasuredSize "boxed_text_uses_aspect_ratio" (Size 10 5) boxedTextUsesAspectRatio
  assertQuickCheckSuccess "flat Halay/Clay conformance" "HALAY_QUICKCHECK_REPLAY" =<< quickCheckWithResult flatArgs (randomLayoutMatchesClay oracle)
  assertQuickCheckSuccess "text Halay/Clay conformance" "HALAY_TEXT_QUICKCHECK_REPLAY" =<< quickCheckWithResult textArgs (randomTextLayoutMatchesClay oracle)
  assertQuickCheckSuccess "tree Halay/Clay conformance" "HALAY_TREE_QUICKCHECK_REPLAY" =<< quickCheckWithResult treeArgs (randomTreeLayoutMatchesClay oracle)

assertQuickCheckSuccess :: String -> String -> Result -> IO ()
assertQuickCheckSuccess name replayVariable result =
  case result of
    Success {} -> pure ()
    Failure {usedSeed, usedSize} ->
      fail
        ( "QuickCheck "
            <> name
            <> " failed; rerun with "
            <> replayVariable
            <> "="
            <> show (show (usedSeed, usedSize))
        )
    _ -> fail ("QuickCheck " <> name <> " failed")

-- <prefix>_TESTS sets the case count; <prefix>_REPLAY takes the
-- "(seed, size)" printed by a failure and makes it the first case run.
fuzzArgs :: Args -> String -> Int -> IO Args
fuzzArgs baseArgs prefix defaultCount = do
  maybeCount <- lookupEnv (prefix <> "_TESTS")
  maybeReplay <- lookupEnv (prefix <> "_REPLAY")
  pure
    baseArgs
      { maxSuccess = fromMaybe defaultCount (maybeCount >>= readMaybe)
      , replay = maybeReplay >>= readMaybe
      }

treeQuickCheckArgs :: Args
treeQuickCheckArgs =
  stdArgs {maxShrinks = 500}

propertyTimeoutMicros :: Int
propertyTimeoutMicros = 1000000

withPropertyTimeout :: String -> IO Property -> IO Property
withPropertyTimeout label action = do
  maybeResult <- timeout propertyTimeoutMicros action
  case maybeResult of
    Just result -> pure result
    Nothing ->
      pure $
        counterexample
          (label <> " timed out after " <> show propertyTimeoutMicros <> " microseconds")
          False

withIOTimeout :: String -> IO value -> IO (Either String value)
withIOTimeout label action = do
  maybeResult <- timeout propertyTimeoutMicros action
  pure
    ( case maybeResult of
        Just result -> Right result
        Nothing -> Left (label <> " timed out after " <> show propertyTimeoutMicros <> " microseconds")
    )

compileClayOracle :: IO FilePath
compileClayOracle = do
  tempDirectory <- getTemporaryDirectory
  let executable = tempDirectory <> "/halay-clay-oracle"
  (compileExit, _compileOut, compileErr) <-
    readProcessWithExitCode
      "cc"
      ["-w", "-std=c99", "halay/test/clay_oracle.c", "-o", executable]
      ""
  case compileExit of
    ExitSuccess -> pure ()
    ExitFailure _ -> error ("Failed to compile Clay oracle:\n" <> compileErr)
  pure executable

runClayOracle :: FilePath -> IO [ClayRect]
runClayOracle executable =
  runClayOracleWithInput executable [] ""

runClayOracleStdin :: FilePath -> String -> IO [ClayRect]
runClayOracleStdin executable =
  runClayOracleWithInput executable ["--stdin"]

runClayOracleTreeStdin :: FilePath -> String -> IO [ClayRect]
runClayOracleTreeStdin executable =
  runClayOracleWithInput executable ["--tree-stdin"]

runClayOracleTextStdin :: FilePath -> String -> IO [ClayRect]
runClayOracleTextStdin executable =
  runClayOracleWithInput executable ["--text-stdin"]

runClayOracleWithInput :: FilePath -> [String] -> String -> IO [ClayRect]
runClayOracleWithInput executable arguments input = do
  (oracleExit, oracleOut, oracleErr) <- readProcessWithExitCode executable arguments input
  case oracleExit of
    ExitSuccess -> pure (parseClayOracle oracleOut)
    ExitFailure _ -> error ("Clay oracle failed:\n" <> oracleErr)

parseClayOracle :: String -> [ClayRect]
parseClayOracle =
  fmap parseLine . lines
  where
    parseLine line =
      case words line of
        [caseName, idName, left, top, rectWidth, rectHeight] ->
          ClayRect
            { clayCase = caseName
            , clayId = idName
            , clayRect =
                Rect
                  (read left)
                  (read top)
                  (read rectWidth)
                  (read rectHeight)
            }
        _ -> error ("Unexpected Clay oracle output: " <> line)

data ConformanceCase = ConformanceCase
  { conformanceName :: String
  , conformanceLayout :: Halay Identity Identity Placements
  }

conformanceCases :: [ConformanceCase]
conformanceCases =
  [ ConformanceCase "row_gap_and_padding" rowGapAndPadding
  , ConformanceCase "column_gap_and_padding" columnGapAndPadding
  , ConformanceCase "fixed_box_centers_child" fixedBoxCentersChild
  , ConformanceCase "percent_child" percentChild
  , ConformanceCase "grow_main_axis" growMainAxis
  , ConformanceCase "grow_cross_axis" growCrossAxis
  , ConformanceCase "clamp_grow" clampGrow
  , ConformanceCase "aspect_ratio_width_drives_height" aspectRatioWidthDrivesHeight
  , ConformanceCase "aspect_ratio_height_drives_width" aspectRatioHeightDrivesWidth
  , ConformanceCase "unequal_grow_main_axis" unequalGrowMainAxis
  , ConformanceCase "nested_box_positions_children" nestedBoxPositionsChildren
  , ConformanceCase "overflow_cross_center" overflowCrossCenter
  , ConformanceCase "clip_main_axis_does_not_compress" clipMainAxisDoesNotCompress
  , ConformanceCase "clip_cross_axis_grows_to_content" clipCrossAxisGrowsToContent
  , ConformanceCase "clip_cross_axis_uses_pre_percent_inner_size" clipCrossAxisUsesPrePercentInnerSize
  , ConformanceCase "clip_child_offset_places_children" clipChildOffsetPlacesChildren
  , ConformanceCase "text_wraps_words" textWrapsWords
  , ConformanceCase "text_respects_newlines" textRespectsNewlines
  ]

assertClayCase :: [ClayRect] -> ConformanceCase -> IO ()
assertClayCase clayRects ConformanceCase {conformanceName, conformanceLayout} =
  assertRectsEqual conformanceName expected actual
  where
    expected = [(clayId rect, clayRect rect) | rect <- clayRects, clayCase rect == conformanceName]
    actual = placedRectsWithRoot conformanceLayout

assertRectsEqual :: String -> [(String, Rect)] -> [(String, Rect)] -> IO ()
assertRectsEqual name expected actual =
  unless (sameLength && and (zipWith sameEntry expected actual)) $
    error
      ( "Halay/Clay conformance failed: "
          <> name
          <> "\nexpected: "
          <> show expected
          <> "\nactual:   "
          <> show actual
      )
  where
    sameLength = length expected == length actual
    sameEntry (expectedName, expectedRect) (actualName, actualRect) =
      expectedName == actualName && nearRect expectedRect actualRect

nearRect :: Rect -> Rect -> Bool
nearRect expected actual =
  near (x expected) (x actual)
    && near (y expected) (y actual)
    && near (width expected) (width actual)
    && near (height expected) (height actual)

near :: Double -> Double -> Bool
near expected actual =
  abs (expected - actual) < 0.05

sameRects :: [(String, Rect)] -> [(String, Rect)] -> Bool
sameRects expected actual =
  length expected == length actual && and (zipWith sameEntry expected actual)
  where
    sameEntry (expectedName, expectedRect) (actualName, actualRect) =
      expectedName == actualName && nearRect expectedRect actualRect

assertMeasuredSize :: String -> Size -> Halay Identity Identity Placements -> IO ()
assertMeasuredSize name expected layout =
  unless (nearSize expected actual) $
    error
      ( "Halay invariant failed: "
          <> name
          <> "\nexpected size: "
          <> show expected
          <> "\nactual size:   "
          <> show actual
      )
  where
    actual = measuredSize (runIdentity (measureHalay layout))

nearSize :: Size -> Size -> Bool
nearSize expected actual =
  near (sizeWidth expected) (sizeWidth actual)
    && near (sizeHeight expected) (sizeHeight actual)

rowGapAndPadding :: Halay Identity Identity Placements
rowGapAndPadding =
  box
    defaultBox
      { boxPadding = Insets 7 3 5 5
      , boxGap = 3
      }
    [named "a" (Size 10 5), named "b" (Size 20 8)]

columnGapAndPadding :: Halay Identity Identity Placements
columnGapAndPadding =
  box
    defaultBox
      { boxDirection = TopToBottom
      , boxPadding = Insets 3 8 10 2
      , boxGap = 4
      }
    [named "a" (Size 10 5), named "b" (Size 20 8)]

fixedBoxCentersChild :: Halay Identity Identity Placements
fixedBoxCentersChild =
  box
    defaultBox
      { boxSizing = Sizing (Fixed 100) (Fixed 50)
      , boxMainAlign = MainCenter
      , boxCrossAlign = CrossCenter
      }
    [named "a" (Size 20 10)]

percentChild :: Halay Identity Identity Placements
percentChild =
  box
    defaultBox {boxSizing = Sizing (Fixed 200) (Fixed 20)}
    [ sized (Sizing (Percent 0.5) (Fixed 10)) (named "a" (Size 10 10))
    , named "b" (Size 20 10)
    ]

growMainAxis :: Halay Identity Identity Placements
growMainAxis =
  box
    defaultBox {boxSizing = Sizing (Fixed 100) (Fixed 20), boxGap = 10}
    [ named "a" (Size 20 10)
    , sized (Sizing (Fill unbounded) (Fixed 10)) (named "b" (Size 0 10))
    ]

growCrossAxis :: Halay Identity Identity Placements
growCrossAxis =
  box
    defaultBox {boxSizing = Sizing (Fixed 100) (Fixed 50)}
    [sized (Sizing (Fixed 10) (Fill unbounded)) (named "a" (Size 10 0))]

clampGrow :: Halay Identity Identity Placements
clampGrow =
  box
    defaultBox {boxSizing = Sizing (Fixed 100) (Fixed 20)}
    [ named "a" (Size 20 10)
    , sized (Sizing (Fill (MinMax Nothing (Just 30))) (Fixed 10)) (named "b" (Size 0 10))
    ]

aspectRatioWidthDrivesHeight :: Halay Identity Identity Placements
aspectRatioWidthDrivesHeight =
  box
    defaultBox {boxSizing = Sizing (Fixed 100) (Fixed 100)}
    [aspectRatio 2 (sized (Sizing (Fixed 40) (Fit unbounded)) (named "a" (Size 0 0)))]

aspectRatioHeightDrivesWidth :: Halay Identity Identity Placements
aspectRatioHeightDrivesWidth =
  box
    defaultBox {boxDirection = TopToBottom, boxSizing = Sizing (Fixed 100) (Fixed 100)}
    [aspectRatio 2 (sized (Sizing (Fit unbounded) (Fixed 30)) (named "a" (Size 0 0)))]

unequalGrowMainAxis :: Halay Identity Identity Placements
unequalGrowMainAxis =
  box
    defaultBox {boxDirection = TopToBottom, boxSizing = Sizing (Fixed 1) (Fixed 4)}
    [ sized (Sizing (Fit unbounded) (Fill unbounded)) (named "a" (Size 1 1))
    , sized (Sizing (Fit unbounded) (Fill unbounded)) (named "b" (Size 1 2))
    ]

nestedBoxPositionsChildren :: Halay Identity Identity Placements
nestedBoxPositionsChildren =
  box
    defaultBox
      { boxSizing = Sizing (Fixed 120) (Fixed 80)
      , boxPadding = Insets 3 7 5 4
      , boxGap = 6
      }
    [ box
        defaultBox
          { boxDirection = TopToBottom
          , boxPadding = Insets 5 2 4 3
          , boxGap = 2
          }
        [named "a" (Size 10 5), named "b" (Size 20 8)]
    , named "c" (Size 15 7)
    ]

overflowCrossCenter :: Halay Identity Identity Placements
overflowCrossCenter =
  box
    defaultBox {boxSizing = Sizing (Fixed 10) (Fixed 10), boxCrossAlign = CrossCenter}
    [named "a" (Size 5 20)]

clipMainAxisDoesNotCompress :: Halay Identity Identity Placements
clipMainAxisDoesNotCompress =
  box
    defaultBox
      { boxSizing = Sizing (Fixed 6) (Fixed 20)
      , boxClip = BoxClip True False (Point 0 0)
      }
    [namedLayout "a" (box defaultBox [text (testTextConfig 1 Nothing) {textPlaceLine = \_ _ _ -> pure mempty} "aaaaa bbbbb"])]

clipCrossAxisGrowsToContent :: Halay Identity Identity Placements
clipCrossAxisGrowsToContent =
  box
    defaultBox
      { boxSizing = Sizing (Fixed 100) (Fixed 10)
      , boxClip = BoxClip False True (Point 0 0)
      }
    [ sized
        (Sizing (Fit unbounded) (Fill unbounded))
        ( namedLayout "a" $
            box
              defaultBox {boxClip = BoxClip False True (Point 0 0)}
              [fixed (Size 5 20) mempty]
        )
    ]

clipCrossAxisUsesPrePercentInnerSize :: Halay Identity Identity Placements
clipCrossAxisUsesPrePercentInnerSize =
  box
    defaultBox
      { boxDirection = TopToBottom
      , boxSizing = Sizing (Fixed 73) (Fixed 80)
      , boxPadding = Insets 0 7 0 12
      , boxCrossAlign = CrossCenter
      , boxClip = BoxClip True True (Point 0 0)
      }
    [ sized
        (Sizing (Fill unbounded) (Fixed 67))
        ( namedLayout "a" $
            box
              defaultBox
                { boxDirection = TopToBottom
                , boxPadding = Insets 5 0 18 0
                , boxClip = BoxClip False True (Point 0 0)
                }
              [text (testTextConfig 1 Nothing) {textWrapMode = TextWrapNone, textAlign = TextAlignCenter, textPlaceLine = \_ _ _ -> pure mempty} "xx xxxxxxxxxxxxxxxxxxx xxxxxxxxxxxxxx xxx"]
        )
    , aspectRatio 1.8 $
        leafWithSizing
          (Sizing (Percent 0.84) (Fixed 31))
          (pure (Size 3 26))
          (\rect -> pure (Placements [("b", rect)]))
    ]

clipChildOffsetPlacesChildren :: Halay Identity Identity Placements
clipChildOffsetPlacesChildren =
  box
    defaultBox
      { boxSizing = Sizing (Fixed 50) (Fixed 50)
      , boxPadding = Insets 6 0 0 5
      , boxClip = BoxClip True True (Point (-3) 7)
      }
    [named "a" (Size 10 10)]

textWrapsWords :: Halay Identity Identity Placements
textWrapsWords =
  box
    defaultBox {boxSizing = Sizing (Fixed 6) (Fixed 20)}
    [text (testTextConfig 1 Nothing) "alpha beta gamma"]

textRespectsNewlines :: Halay Identity Identity Placements
textRespectsNewlines =
  box
    defaultBox {boxSizing = Sizing (Fixed 20) (Fixed 20)}
    [text (testTextConfig 1 Nothing) {textWrapMode = TextWrapNewlines} "alpha\nbeta"]

rawTextIgnoresAspectRatio :: Halay Identity Identity Placements
rawTextIgnoresAspectRatio =
  aspectRatio 2 (text (testTextConfig 1 Nothing) "alpha beta")

boxedTextUsesAspectRatio :: Halay Identity Identity Placements
boxedTextUsesAspectRatio =
  aspectRatio 2 $
    box
      defaultBox {boxSizing = Sizing (Fixed 10) (Fit unbounded)}
      [text (testTextConfig 1 Nothing) "alpha beta"]

testTextConfig :: Int -> Maybe Int -> TextConfig Identity Identity Placements
testTextConfig fontSize maybeLineHeight =
  TextConfig
    { textLineHeight = fromIntegral <$> maybeLineHeight
    , textWrapMode = TextWrapWords
    , textAlign = TextAlignStart
    , textMeasure = \string -> pure (Size (fromIntegral (length string * fontSize)) (fromIntegral fontSize))
    , textPlaceLine = \index _line rect -> pure (Placements [("text" <> show index, rect)])
    }

data RandomLayout = RandomLayout
  { randomDirection :: Direction
  , randomPadding :: Insets
  , randomGap :: Int
  , randomAlignX :: AlignChoice
  , randomAlignY :: AlignChoice
  , randomRootSize :: Size
  , randomChildren :: [RandomChild]
  }
  deriving (Eq, Show)

data RandomChild = RandomChild
  { randomChildSize :: Size
  , randomChildSizing :: Sizing
  , randomChildAspectRatio :: Maybe Double
  }
  deriving (Eq, Show)

instance Arbitrary RandomLayout where
  arbitrary = do
    direction <- elements [LeftToRight, TopToBottom]
    paddingLeft <- chooseInt (0, 20)
    paddingRight <- chooseInt (0, 20)
    paddingTop <- chooseInt (0, 20)
    paddingBottom <- chooseInt (0, 20)
    gap <- chooseInt (0, 16)
    alignX <- arbitrary
    alignY <- arbitrary
    rootWidth <- chooseInt (40, 220)
    rootHeight <- chooseInt (30, 160)
    childCount <- chooseInt (1, 4)
    children <- vectorOf childCount arbitraryChild
    pure
      RandomLayout
        { randomDirection = direction
        , randomPadding =
            Insets
              { insetTop = fromIntegral paddingTop
              , insetRight = fromIntegral paddingRight
              , insetBottom = fromIntegral paddingBottom
              , insetLeft = fromIntegral paddingLeft
              }
        , randomGap = gap
        , randomAlignX = alignX
        , randomAlignY = alignY
        , randomRootSize = Size (fromIntegral rootWidth) (fromIntegral rootHeight)
        , randomChildren = children
        }
  shrink randomLayout =
    [randomLayout {randomDirection = direction} | direction <- shrinkDirection (randomDirection randomLayout)]
      <> [randomLayout {randomPadding = insets} | insets <- shrinkInsets (randomPadding randomLayout)]
      <> [randomLayout {randomGap = gap} | gap <- shrinkIntAtLeast 0 (randomGap randomLayout)]
      <> [randomLayout {randomAlignX = alignX} | alignX <- shrink (randomAlignX randomLayout)]
      <> [randomLayout {randomAlignY = alignY} | alignY <- shrink (randomAlignY randomLayout)]
      <> [randomLayout {randomRootSize = rootSize} | rootSize <- shrinkSizeAtLeast 1 1 (randomRootSize randomLayout)]
      <> [randomLayout {randomChildren = children} | children <- shrinkChildren (randomChildren randomLayout)]

arbitraryChild :: Gen RandomChild
arbitraryChild = do
  childWidth <- chooseInt (1, 80)
  childHeight <- chooseInt (1, 60)
  childWidthSizing <- arbitraryAxisSizing
  childHeightSizing <- arbitraryAxisSizing
  aspect <- arbitraryAspectRatio
  pure
    RandomChild
      { randomChildSize = Size (fromIntegral childWidth) (fromIntegral childHeight)
      , randomChildSizing = Sizing childWidthSizing childHeightSizing
      , randomChildAspectRatio = aspect
      }

data AlignChoice
  = AlignStart
  | AlignCenter
  | AlignEnd
  deriving (Eq, Show)

instance Arbitrary AlignChoice where
  arbitrary =
    elements [AlignStart, AlignCenter, AlignEnd]
  shrink AlignStart = []
  shrink AlignCenter = [AlignStart]
  shrink AlignEnd = [AlignStart, AlignCenter]

shrinkDirection :: Direction -> [Direction]
shrinkDirection LeftToRight = []
shrinkDirection TopToBottom = [LeftToRight]

shrinkInsets :: Insets -> [Insets]
shrinkInsets Insets {insetTop, insetRight, insetBottom, insetLeft} =
  [Insets value insetRight insetBottom insetLeft | value <- shrinkDoubleAtLeast 0 insetTop]
    <> [Insets insetTop value insetBottom insetLeft | value <- shrinkDoubleAtLeast 0 insetRight]
    <> [Insets insetTop insetRight value insetLeft | value <- shrinkDoubleAtLeast 0 insetBottom]
    <> [Insets insetTop insetRight insetBottom value | value <- shrinkDoubleAtLeast 0 insetLeft]

shrinkSizeAtLeast :: Int -> Int -> Size -> [Size]
shrinkSizeAtLeast minWidth minHeight Size {sizeWidth, sizeHeight} =
  [Size value sizeHeight | value <- shrinkDoubleAtLeast minWidth sizeWidth]
    <> [Size sizeWidth value | value <- shrinkDoubleAtLeast minHeight sizeHeight]

shrinkChildren :: [RandomChild] -> [[RandomChild]]
shrinkChildren =
  filter (not . null) . shrinkList shrinkChild

shrinkChild :: RandomChild -> [RandomChild]
shrinkChild child@RandomChild {randomChildSize, randomChildSizing, randomChildAspectRatio} =
  [child {randomChildAspectRatio = Nothing} | randomChildAspectRatio /= Nothing]
    <> [child {randomChildSizing = sizing} | sizing <- shrinkSizing randomChildSizing]
    <> [child {randomChildSize = childSize} | childSize <- shrinkSizeAtLeast 1 1 randomChildSize]
    <> [child {randomChildAspectRatio = Just ratio} | ratio <- shrinkAspectRatio randomChildAspectRatio]

arbitraryAxisSizing :: Gen AxisSizing
arbitraryAxisSizing = do
  sizingChoice <- chooseInt (0, 9)
  case sizingChoice of
    0 -> pure (Fit unbounded)
    1 -> Fixed . fromIntegral <$> chooseInt (1, 80)
    2 -> pure (Fill unbounded)
    3 -> Percent . (/ 100) . fromIntegral <$> chooseInt (1, 100)
    4 -> Fill <$> arbitraryMaxOnly
    5 -> Fill <$> arbitraryMinOnly
    6 -> Fill <$> arbitraryMinMax
    7 -> Fit <$> arbitraryMaxOnly
    8 -> Fit <$> arbitraryMinOnly
    9 -> Fit <$> arbitraryMinMax
    _ -> Fixed . fromIntegral <$> chooseInt (1, 80)
  where
    arbitraryMaxOnly = do
      maximumValue <- chooseInt (1, 80)
      pure (MinMax Nothing (Just (fromIntegral maximumValue)))
    arbitraryMinOnly = do
      minimumValue <- chooseInt (1, 40)
      pure (MinMax (Just (fromIntegral minimumValue)) Nothing)
    arbitraryMinMax = do
      minimumValue <- chooseInt (1, 40)
      maximumValue <- chooseInt (minimumValue, 80)
      pure (MinMax (Just (fromIntegral minimumValue)) (Just (fromIntegral maximumValue)))

shrinkSizing :: Sizing -> [Sizing]
shrinkSizing Sizing {sizingWidth, sizingHeight} =
  [Sizing width sizingHeight | width <- shrinkAxisSizing sizingWidth]
    <> [Sizing sizingWidth height | height <- shrinkAxisSizing sizingHeight]

shrinkAxisSizing :: AxisSizing -> [AxisSizing]
shrinkAxisSizing (Fit (MinMax Nothing Nothing)) = []
shrinkAxisSizing (Fit minMax) =
  Fit unbounded : [Fit shrunk | shrunk <- shrinkMinMax minMax]
shrinkAxisSizing (Fixed value) =
  Fit unbounded : [Fixed shrunk | shrunk <- shrinkDoubleAtLeast 1 value]
shrinkAxisSizing (Fill minMax) =
  Fit unbounded : Fit minMax : [Fill shrunk | shrunk <- shrinkMinMax minMax]
shrinkAxisSizing (Percent value) =
  Fit unbounded : [Percent shrunk | NonNegative shrunk <- shrink (NonNegative value), shrunk > 0, shrunk <= 1]

shrinkMinMax :: MinMax -> [MinMax]
shrinkMinMax (MinMax maybeMin maybeMax) =
  [MinMax Nothing maybeMax | Just _ <- [maybeMin]]
    <> [MinMax maybeMin Nothing | Just _ <- [maybeMax]]

shrinkAspectRatio :: Maybe Double -> [Double]
shrinkAspectRatio Nothing = []
shrinkAspectRatio (Just ratio) =
  [value | NonNegative value <- shrink (NonNegative ratio), value > 0]

shrinkDoubleAtLeast :: Int -> Double -> [Double]
shrinkDoubleAtLeast minimumValue value =
  fromIntegral <$> shrinkIntAtLeast minimumValue (round value)

shrinkIntAtLeast :: Int -> Int -> [Int]
shrinkIntAtLeast minimumValue value =
  filter (>= minimumValue) (shrink value)

randomLayoutMatchesClay :: FilePath -> RandomLayout -> Property
randomLayoutMatchesClay oracle randomLayout =
  ioProperty $ withPropertyTimeout "flat Halay/Clay conformance case" $ do
    clayRects <- runClayOracleStdin oracle (randomLayoutOracleInput randomLayout)
    let expected = [(clayId rect, clayRect rect) | rect <- clayRects, clayCase rect == "quickcheck"]
    let actual = placedRectsWithRoot (randomLayoutHalay randomLayout)
    ok <- evaluate (sameRects expected actual)
    pure $
      counterexample
        ( "layout:   "
            <> show randomLayout
            <> "\nexpected: "
            <> show expected
            <> "\nactual:   "
            <> show actual
        )
        ok

randomLayoutHalay :: RandomLayout -> Halay Identity Identity Placements
randomLayoutHalay randomLayout =
  box
    defaultBox
      { boxDirection = randomDirection randomLayout
      , boxPadding = randomPadding randomLayout
      , boxGap = fromIntegral (randomGap randomLayout)
      , boxSizing =
          Sizing
            (Fixed (sizeWidth (randomRootSize randomLayout)))
            (Fixed (sizeHeight (randomRootSize randomLayout)))
      , boxMainAlign = randomMainAlign randomLayout
      , boxCrossAlign = randomCrossAlign randomLayout
      }
    [randomChildHalay childName child | (childName, child) <- zip childNames (randomChildren randomLayout)]

randomChildHalay :: String -> RandomChild -> Halay Identity Identity Placements
randomChildHalay childName RandomChild {randomChildSize, randomChildSizing, randomChildAspectRatio} =
  case randomChildAspectRatio of
    Nothing -> sized randomChildSizing (named childName randomChildSize)
    Just ratio -> aspectRatio ratio (sized randomChildSizing (named childName randomChildSize))

randomMainAlign :: RandomLayout -> MainAlign
randomMainAlign RandomLayout {randomDirection = LeftToRight, randomAlignX} =
  mainAlign randomAlignX
randomMainAlign RandomLayout {randomDirection = TopToBottom, randomAlignY} =
  mainAlign randomAlignY

randomCrossAlign :: RandomLayout -> CrossAlign
randomCrossAlign RandomLayout {randomDirection = LeftToRight, randomAlignY} =
  crossAlign randomAlignY
randomCrossAlign RandomLayout {randomDirection = TopToBottom, randomAlignX} =
  crossAlign randomAlignX

mainAlign :: AlignChoice -> MainAlign
mainAlign AlignStart = MainStart
mainAlign AlignCenter = MainCenter
mainAlign AlignEnd = MainEnd

crossAlign :: AlignChoice -> CrossAlign
crossAlign AlignStart = CrossStart
crossAlign AlignCenter = CrossCenter
crossAlign AlignEnd = CrossEnd

randomLayoutOracleInput :: RandomLayout -> String
randomLayoutOracleInput randomLayout =
  unwords
    ( [ "quickcheck"
      , show (directionValue (randomDirection randomLayout))
      , show (round (insetLeft randomInsets) :: Int)
      , show (round (insetRight randomInsets) :: Int)
      , show (round (insetTop randomInsets) :: Int)
      , show (round (insetBottom randomInsets) :: Int)
      , show (randomGap randomLayout)
      , show (alignValue (randomAlignX randomLayout))
      , show (alignValue (randomAlignY randomLayout))
      , show (sizeWidth rootSize)
      , show (sizeHeight rootSize)
      , show (length children)
      ]
        <> concatMap childWords children
    )
    <> "\n"
  where
    randomInsets = randomPadding randomLayout
    rootSize = randomRootSize randomLayout
    children = randomChildren randomLayout
    childWords RandomChild {randomChildSize = Size {sizeWidth, sizeHeight}, randomChildSizing, randomChildAspectRatio} =
      [ show sizeWidth
      , show sizeHeight
      , show (axisSizingValueType (sizingWidth randomChildSizing))
      , show (axisSizingValueType (sizingHeight randomChildSizing))
      , show (axisSizingValue (sizingWidth randomChildSizing))
      , show (axisSizingValue (sizingHeight randomChildSizing))
      , show (axisSizingMin (sizingWidth randomChildSizing))
      , show (axisSizingMin (sizingHeight randomChildSizing))
      , show (axisSizingMax (sizingWidth randomChildSizing))
      , show (axisSizingMax (sizingHeight randomChildSizing))
      , maybe "0" show randomChildAspectRatio
      ]

data RandomTextLayout = RandomTextLayout
  { randomTextRootWidth :: Int
  , randomTextRootHeight :: Int
  , randomTextWrapMode :: TextWrapMode
  , randomTextAlign :: TextAlign
  , randomTextFontSize :: Int
  , randomTextLineHeight :: Maybe Int
  , randomTextLineWordLengths :: [[Int]]
  }
  deriving (Eq, Show)

instance Arbitrary RandomTextLayout where
  arbitrary = do
    rootWidth <- chooseInt (1, 60)
    rootHeight <- chooseInt (1, 80)
    wrapMode <- arbitraryTextWrapMode
    textAlign <- arbitraryTextAlign
    fontSize <- chooseInt (1, 5)
    lineHeight <- arbitraryTextLineHeight fontSize
    lineCount <-
      case wrapMode of
        TextWrapNewlines -> chooseInt (1, 4)
        _ -> pure 1
    lineWordLengths <- vectorOf lineCount arbitraryWordLengths
    pure
      RandomTextLayout
        { randomTextRootWidth = rootWidth
        , randomTextRootHeight = rootHeight
        , randomTextWrapMode = wrapMode
        , randomTextAlign = textAlign
        , randomTextFontSize = fontSize
        , randomTextLineHeight = lineHeight
        , randomTextLineWordLengths = lineWordLengths
        }
  shrink randomTextLayout =
    [randomTextLayout {randomTextRootWidth = value} | value <- shrinkIntAtLeast 1 (randomTextRootWidth randomTextLayout)]
      <> [randomTextLayout {randomTextRootHeight = value} | value <- shrinkIntAtLeast 1 (randomTextRootHeight randomTextLayout)]
      <> [randomTextLayout {randomTextWrapMode = wrapMode} | wrapMode <- shrinkTextWrapMode (randomTextWrapMode randomTextLayout)]
      <> [randomTextLayout {randomTextAlign = textAlign} | textAlign <- shrinkTextAlign (randomTextAlign randomTextLayout)]
      <> [randomTextLayout {randomTextFontSize = value} | value <- shrinkIntAtLeast 1 (randomTextFontSize randomTextLayout)]
      <> [randomTextLayout {randomTextLineHeight = value} | value <- shrinkTextLineHeight (randomTextLineHeight randomTextLayout)]
      <> [randomTextLayout {randomTextLineWordLengths = lengths} | lengths <- shrinkLineWordLengths (randomTextLineWordLengths randomTextLayout)]

arbitraryTextWrapMode :: Gen TextWrapMode
arbitraryTextWrapMode =
  elements [TextWrapWords, TextWrapNewlines, TextWrapNone]

shrinkTextWrapMode :: TextWrapMode -> [TextWrapMode]
shrinkTextWrapMode TextWrapWords = []
shrinkTextWrapMode TextWrapNewlines = [TextWrapWords]
shrinkTextWrapMode TextWrapNone = [TextWrapWords]

arbitraryTextAlign :: Gen TextAlign
arbitraryTextAlign =
  elements [TextAlignStart, TextAlignCenter, TextAlignEnd]

shrinkTextAlign :: TextAlign -> [TextAlign]
shrinkTextAlign TextAlignStart = []
shrinkTextAlign TextAlignCenter = [TextAlignStart]
shrinkTextAlign TextAlignEnd = [TextAlignStart, TextAlignCenter]

arbitraryTextLineHeight :: Int -> Gen (Maybe Int)
arbitraryTextLineHeight fontSize =
  frequency
    [ (3, pure Nothing)
    , (1, Just <$> chooseInt (1, fontSize * 3))
    ]

shrinkTextLineHeight :: Maybe Int -> [Maybe Int]
shrinkTextLineHeight Nothing = []
shrinkTextLineHeight (Just value) =
  Nothing : [Just shrunk | shrunk <- shrinkIntAtLeast 1 value]

arbitraryWordLengths :: Gen [Int]
arbitraryWordLengths = do
  wordCount <- chooseInt (1, 8)
  vectorOf wordCount (chooseInt (1, 20))

shrinkLineWordLengths :: [[Int]] -> [[[Int]]]
shrinkLineWordLengths =
  filter (not . null) . shrinkList shrinkWordLengths

shrinkWordLengths :: [Int] -> [[Int]]
shrinkWordLengths =
  filter (not . null) . shrinkList (shrinkIntAtLeast 1)

randomTextLayoutMatchesClay :: FilePath -> RandomTextLayout -> Property
randomTextLayoutMatchesClay oracle randomTextLayout =
  ioProperty $ withPropertyTimeout "text Halay/Clay conformance case" $ do
    clayRects <- runClayOracleTextStdin oracle (randomTextOracleInput randomTextLayout)
    let expected = [(clayId rect, clayRect rect) | rect <- clayRects, clayCase rect == "textcheck"]
    let actual = placedRectsWithRoot (randomTextLayoutHalay randomTextLayout)
    ok <- evaluate (sameRects expected actual)
    pure $
      counterexample
        ( "layout:   "
            <> show randomTextLayout
            <> "\nexpected: "
            <> show expected
            <> "\nactual:   "
            <> show actual
        )
        ok

randomTextLayoutHalay :: RandomTextLayout -> Halay Identity Identity Placements
randomTextLayoutHalay RandomTextLayout {randomTextRootWidth, randomTextRootHeight, randomTextWrapMode, randomTextAlign, randomTextFontSize, randomTextLineHeight, randomTextLineWordLengths} =
  box
    defaultBox
      { boxSizing =
          Sizing
            (Fixed (fromIntegral randomTextRootWidth))
            (Fixed (fromIntegral randomTextRootHeight))
      }
    [ text
        (testTextConfig randomTextFontSize randomTextLineHeight)
          { textWrapMode = randomTextWrapMode
          , textAlign = randomTextAlign
          }
        (linesText randomTextLineWordLengths)
    ]

randomTextOracleInput :: RandomTextLayout -> String
randomTextOracleInput RandomTextLayout {randomTextRootWidth, randomTextRootHeight, randomTextWrapMode, randomTextAlign, randomTextFontSize, randomTextLineHeight, randomTextLineWordLengths} =
  unwords
    ( [ "textcheck"
      , show randomTextRootWidth
      , show randomTextRootHeight
      , show (textWrapModeValue randomTextWrapMode)
      , show (textAlignValue randomTextAlign)
      , show randomTextFontSize
      , show (fromMaybe 0 randomTextLineHeight)
      , show (length randomTextLineWordLengths)
      ]
        <> concatMap lineWords randomTextLineWordLengths
    )
    <> "\n"
  where
    lineWords wordLengths =
      show (length wordLengths) : (show <$> wordLengths)

wordsText :: [Int] -> String
wordsText lengths =
  intercalate " " [replicate lengthValue 'x' | lengthValue <- lengths]

linesText :: [[Int]] -> String
linesText lineLengths =
  intercalate "\n" (wordsText <$> lineLengths)

textWrapModeValue :: TextWrapMode -> Int
textWrapModeValue TextWrapWords = 0
textWrapModeValue TextWrapNewlines = 1
textWrapModeValue TextWrapNone = 2

textAlignValue :: TextAlign -> Int
textAlignValue TextAlignStart = 0
textAlignValue TextAlignCenter = 1
textAlignValue TextAlignEnd = 2

data RandomTreeLayout = RandomTreeLayout
  { randomTreeRootConfig :: RandomBoxConfig
  , randomTreeRootSize :: Size
  , randomTreeChildren :: [RandomTreeNode]
  }
  deriving (Eq, Show)

data RandomTreeNode
  = RandomTreeLeaf Size Sizing (Maybe Double)
  | RandomTreeText RandomBoxConfig (Maybe Double) RandomTreeTextContent
  | RandomTreeBox RandomBoxConfig (Maybe Double) [RandomTreeNode]
  deriving (Eq, Show)

data RandomTreeTextContent = RandomTreeTextContent
  { randomTreeTextWrapMode :: TextWrapMode
  , randomTreeTextAlign :: TextAlign
  , randomTreeTextFontSize :: Int
  , randomTreeTextLineHeight :: Maybe Int
  , randomTreeTextLineWordLengths :: [[Int]]
  }
  deriving (Eq, Show)

data RandomBoxConfig = RandomBoxConfig
  { randomBoxDirection :: Direction
  , randomBoxPadding :: Insets
  , randomBoxGap :: Int
  , randomBoxAlignX :: AlignChoice
  , randomBoxAlignY :: AlignChoice
  , randomBoxSizing :: Sizing
  , randomBoxClipHorizontal :: Bool
  , randomBoxClipVertical :: Bool
  , randomBoxChildOffset :: Point
  }
  deriving (Eq, Show)

instance Arbitrary RandomTreeLayout where
  arbitrary = do
    rootWidth <- chooseInt (40, 220)
    rootHeight <- chooseInt (30, 160)
    childCount <- chooseInt (1, 4)
    let rootSize = Size (fromIntegral rootWidth) (fromIntegral rootHeight)
    rootConfig <-
      arbitraryBoxConfigWithSizing
        childCount
        (Sizing (Fixed (sizeWidth rootSize)) (Fixed (sizeHeight rootSize)))
    children <- vectorOf childCount (arbitraryTreeNode 3)
    pure
      RandomTreeLayout
        { randomTreeRootConfig = rootConfig
        , randomTreeRootSize = rootSize
        , randomTreeChildren = children
        }
  shrink randomTree =
    filter
      oracleSafeRandomTree
      ( [randomTree {randomTreeRootConfig = config} | config <- shrinkBoxConfig (length (randomTreeChildren randomTree)) (rootSizedConfig randomTree)]
          <> [randomTree {randomTreeRootSize = size} | size <- shrinkSizeAtLeast 1 1 (randomTreeRootSize randomTree)]
          <> [randomTree {randomTreeChildren = children} | children <- shrinkTreeChildren (randomTreeChildren randomTree)]
      )

arbitraryTreeNode :: Int -> Gen RandomTreeNode
arbitraryTreeNode depth
  | depth <= 0 =
      frequency
        [ (3, arbitraryTreeLeaf)
        , (2, arbitraryTreeText)
        ]
  | otherwise =
      frequency
        [ (3, arbitraryTreeLeaf)
        , (2, arbitraryTreeText)
        , (4, arbitraryTreeBox depth)
        ]

arbitraryTreeLeaf :: Gen RandomTreeNode
arbitraryTreeLeaf = do
  width <- chooseInt (1, 80)
  height <- chooseInt (1, 60)
  widthSizing <- arbitraryTreeAxisSizing
  heightSizing <- arbitraryTreeAxisSizing
  aspect <- arbitraryAspectRatio
  pure (RandomTreeLeaf (Size (fromIntegral width) (fromIntegral height)) (Sizing widthSizing heightSizing) aspect)

arbitraryTreeText :: Gen RandomTreeNode
arbitraryTreeText = do
  config <- arbitraryTreeBoxConfig 1
  aspect <- arbitraryAspectRatio
  textContent <- arbitraryTreeTextContent
  pure (RandomTreeText config aspect textContent)

arbitraryTreeTextContent :: Gen RandomTreeTextContent
arbitraryTreeTextContent = do
  wrapMode <- arbitraryTextWrapMode
  textAlign <- arbitraryTextAlign
  fontSize <- chooseInt (1, 5)
  lineHeight <- arbitraryTextLineHeight fontSize
  lineCount <-
    case wrapMode of
      TextWrapNewlines -> chooseInt (1, 4)
      _ -> pure 1
  lineWordLengths <- vectorOf lineCount arbitraryWordLengths
  pure
    RandomTreeTextContent
      { randomTreeTextWrapMode = wrapMode
      , randomTreeTextAlign = textAlign
      , randomTreeTextFontSize = fontSize
      , randomTreeTextLineHeight = lineHeight
      , randomTreeTextLineWordLengths = lineWordLengths
      }

arbitraryTreeBox :: Int -> Gen RandomTreeNode
arbitraryTreeBox depth = do
  childCount <- chooseInt (1, 4)
  config <- arbitraryTreeBoxConfig childCount
  aspect <- arbitraryAspectRatio
  children <- vectorOf childCount (arbitraryTreeNode (depth - 1))
  pure (RandomTreeBox config aspect children)

arbitraryTreeBoxConfig :: Int -> Gen RandomBoxConfig
arbitraryTreeBoxConfig childCount = do
  widthSizing <- arbitraryTreeAxisSizing
  heightSizing <- arbitraryTreeAxisSizing
  arbitraryBoxConfigWithSizing childCount (Sizing widthSizing heightSizing)

arbitraryTreeAxisSizing :: Gen AxisSizing
arbitraryTreeAxisSizing = arbitraryAxisSizing

arbitraryAspectRatio :: Gen (Maybe Double)
arbitraryAspectRatio =
  frequency
    [ (4, pure Nothing)
    , (1, Just . (/ 10) . fromIntegral <$> chooseInt (5, 40))
    ]

arbitraryBoxConfigWithSizing :: Int -> Sizing -> Gen RandomBoxConfig
arbitraryBoxConfigWithSizing childCount sizing = do
  direction <- elements [LeftToRight, TopToBottom]
  gap <- arbitraryGapForMainAxis childCount direction sizing
  let horizontalGap = axisReservedGap Horizontal childCount direction gap
  let verticalGap = axisReservedGap Vertical childCount direction gap
  (paddingLeft, paddingRight) <- arbitraryPaddingPairForAxis (sizingWidth sizing) horizontalGap
  (paddingTop, paddingBottom) <- arbitraryPaddingPairForAxis (sizingHeight sizing) verticalGap
  alignX <- arbitrary
  alignY <- arbitrary
  clipHorizontal <- arbitraryClipFlag
  clipVertical <- arbitraryClipFlag
  childOffset <- arbitraryChildOffset
  pure
    RandomBoxConfig
      { randomBoxDirection = direction
      , randomBoxPadding =
          Insets
            { insetTop = fromIntegral paddingTop
            , insetRight = fromIntegral paddingRight
            , insetBottom = fromIntegral paddingBottom
            , insetLeft = fromIntegral paddingLeft
            }
      , randomBoxGap = gap
      , randomBoxAlignX = alignX
      , randomBoxAlignY = alignY
      , randomBoxSizing = sizing
      , randomBoxClipHorizontal = clipHorizontal
      , randomBoxClipVertical = clipVertical
      , randomBoxChildOffset = childOffset
      }

arbitraryClipFlag :: Gen Bool
arbitraryClipFlag =
  frequency [(3, pure False), (1, pure True)]

arbitraryChildOffset :: Gen Point
arbitraryChildOffset =
  Point
    . fromIntegral
    <$> chooseInt (-20, 20)
    <*> (fromIntegral <$> chooseInt (-20, 20))

arbitraryPaddingPair :: Maybe Double -> Gen (Int, Int)
arbitraryPaddingPair maybeMaxTotal =
  case maybeMaxTotal of
    Nothing -> (,) <$> chooseInt (0, 20) <*> chooseInt (0, 20)
    Just maxTotal
      | maxTotal <= 0 -> pure (0, 0)
      | otherwise -> do
          first <- chooseInt (0, min 20 (floor maxTotal))
          second <- chooseInt (0, min 20 (floor maxTotal - first))
          pure (first, second)

arbitraryPaddingPairForAxis :: AxisSizing -> Double -> Gen (Int, Int)
arbitraryPaddingPairForAxis sizing reserved =
  case fixedAxisSize sizing of
    Just maxTotal -> arbitraryPaddingPair (Just (max 0 (maxTotal - reserved)))
    Nothing -> pure (0, 0)

arbitraryGapForMainAxis :: Int -> Direction -> Sizing -> Gen Int
arbitraryGapForMainAxis childCount direction sizing =
  case fixedAxisSize (mainAxisSizing direction sizing) of
    Just fixedSize
      | childCount > 1 -> chooseInt (0, min 16 (floor (fixedSize / fromIntegral (childCount - 1))))
    _ -> pure 0

shrinkTreeChildren :: [RandomTreeNode] -> [[RandomTreeNode]]
shrinkTreeChildren =
  filter (not . null) . shrinkList shrinkTreeNode

shrinkTreeNode :: RandomTreeNode -> [RandomTreeNode]
shrinkTreeNode (RandomTreeLeaf size sizing aspect) =
  [RandomTreeLeaf shrunkSize sizing aspect | shrunkSize <- shrinkSizeAtLeast 1 1 size]
    <> [RandomTreeLeaf size shrunkSizing aspect | shrunkSizing <- shrinkSizing sizing]
    <> [RandomTreeLeaf size sizing Nothing | aspect /= Nothing]
    <> [RandomTreeLeaf size sizing (Just ratio) | ratio <- shrinkAspectRatio aspect]
shrinkTreeNode (RandomTreeText config aspect textContent) =
  [RandomTreeText shrunkConfig aspect textContent | shrunkConfig <- shrinkBoxConfig 1 config]
    <> [RandomTreeText config Nothing textContent | aspect /= Nothing]
    <> [RandomTreeText config (Just ratio) textContent | ratio <- shrinkAspectRatio aspect]
    <> [RandomTreeText config aspect shrunkTextContent | shrunkTextContent <- shrinkTreeTextContent textContent]
shrinkTreeNode (RandomTreeBox config aspect children) =
  children
    <> [RandomTreeBox shrunkConfig aspect children | shrunkConfig <- shrinkBoxConfig (length children) config]
    <> [RandomTreeBox config Nothing children | aspect /= Nothing]
    <> [RandomTreeBox config (Just ratio) children | ratio <- shrinkAspectRatio aspect]
    <> [RandomTreeBox config aspect shrunkChildren | shrunkChildren <- shrinkTreeChildren children]

shrinkTreeTextContent :: RandomTreeTextContent -> [RandomTreeTextContent]
shrinkTreeTextContent textContent =
  [textContent {randomTreeTextWrapMode = wrapMode} | wrapMode <- shrinkTextWrapMode (randomTreeTextWrapMode textContent)]
    <> [textContent {randomTreeTextAlign = textAlign} | textAlign <- shrinkTextAlign (randomTreeTextAlign textContent)]
    <> [textContent {randomTreeTextFontSize = value} | value <- shrinkIntAtLeast 1 (randomTreeTextFontSize textContent)]
    <> [textContent {randomTreeTextLineHeight = value} | value <- shrinkTextLineHeight (randomTreeTextLineHeight textContent)]
    <> [textContent {randomTreeTextLineWordLengths = lengths} | lengths <- shrinkLineWordLengths (randomTreeTextLineWordLengths textContent)]

shrinkBoxConfig :: Int -> RandomBoxConfig -> [RandomBoxConfig]
shrinkBoxConfig childCount config =
  filter
    (oracleSafeBoxConfig childCount)
    ( [config {randomBoxDirection = direction} | direction <- shrinkDirection (randomBoxDirection config)]
        <> [config {randomBoxPadding = insets} | insets <- shrinkInsets (randomBoxPadding config)]
        <> [config {randomBoxGap = gap} | gap <- shrinkIntAtLeast 0 (randomBoxGap config)]
        <> [config {randomBoxAlignX = alignX} | alignX <- shrink (randomBoxAlignX config)]
        <> [config {randomBoxAlignY = alignY} | alignY <- shrink (randomBoxAlignY config)]
        <> [config {randomBoxSizing = sizing} | sizing <- shrinkSizing (randomBoxSizing config)]
        <> [config {randomBoxClipHorizontal = False} | randomBoxClipHorizontal config]
        <> [config {randomBoxClipVertical = False} | randomBoxClipVertical config]
        <> [config {randomBoxChildOffset = offset} | offset <- shrinkPoint (randomBoxChildOffset config)]
    )

shrinkPoint :: Point -> [Point]
shrinkPoint Point {pointX, pointY} =
  [Point value pointY | value <- shrinkDouble pointX]
    <> [Point pointX value | value <- shrinkDouble pointY]

shrinkDouble :: Double -> [Double]
shrinkDouble value =
  fromIntegral <$> shrink (round value :: Int)

oracleSafeRandomTree :: RandomTreeLayout -> Bool
oracleSafeRandomTree randomTree =
  oracleSafeBoxConfig (length (randomTreeChildren randomTree)) (rootSizedConfig randomTree)
    && all oracleSafeTreeNode (randomTreeChildren randomTree)

oracleSafeTreeNode :: RandomTreeNode -> Bool
oracleSafeTreeNode RandomTreeLeaf {} = True
oracleSafeTreeNode (RandomTreeText config _aspect _textContent) =
  oracleSafeBoxConfig 1 config
oracleSafeTreeNode (RandomTreeBox config _aspect children) =
  oracleSafeBoxConfig (length children) config && all oracleSafeTreeNode children

oracleSafeBoxConfig :: Int -> RandomBoxConfig -> Bool
oracleSafeBoxConfig childCount RandomBoxConfig {randomBoxDirection, randomBoxPadding, randomBoxGap, randomBoxSizing} =
  paddingFitsAxis (sizingWidth randomBoxSizing) (insetLeft randomBoxPadding + insetRight randomBoxPadding + axisReservedGap Horizontal childCount randomBoxDirection randomBoxGap)
    && paddingFitsAxis (sizingHeight randomBoxSizing) (insetTop randomBoxPadding + insetBottom randomBoxPadding + axisReservedGap Vertical childCount randomBoxDirection randomBoxGap)

paddingFitsAxis :: AxisSizing -> Double -> Bool
paddingFitsAxis sizing paddingSize =
  case fixedAxisSize sizing of
    Just maxSize -> paddingSize <= maxSize
    Nothing -> paddingSize == 0

data Axis
  = Horizontal
  | Vertical

mainAxisSizing :: Direction -> Sizing -> AxisSizing
mainAxisSizing LeftToRight = sizingWidth
mainAxisSizing TopToBottom = sizingHeight

axisReservedGap :: Axis -> Int -> Direction -> Int -> Double
axisReservedGap axis childCount direction gap
  | childCount <= 1 = 0
  | axisIsAlongDirection axis direction =
      fromIntegral (childCount - 1) * fromIntegral gap
  | otherwise = 0

axisIsAlongDirection :: Axis -> Direction -> Bool
axisIsAlongDirection Horizontal LeftToRight = True
axisIsAlongDirection Vertical TopToBottom = True
axisIsAlongDirection _ _ = False

rootSizedConfig :: RandomTreeLayout -> RandomBoxConfig
rootSizedConfig RandomTreeLayout {randomTreeRootConfig, randomTreeRootSize} =
  randomTreeRootConfig
    { randomBoxSizing =
        Sizing
          (Fixed (sizeWidth randomTreeRootSize))
          (Fixed (sizeHeight randomTreeRootSize))
    }

randomTreeLayoutMatchesClay :: FilePath -> RandomTreeLayout -> Property
randomTreeLayoutMatchesClay oracle randomTree =
  ioProperty $ do
    let input = randomTreeOracleInput randomTree
    maybeClayRects <- withIOTimeout "Clay tree oracle" (runClayOracleTreeStdin oracle input)
    case maybeClayRects of
      Left timeoutMessage ->
        pure $
          counterexample
            ( timeoutMessage
                <> "\nlayout:   "
                <> show randomTree
                <> "\ninput:    "
                <> input
            )
            False
      Right clayRects -> do
        let expected = [(clayId rect, clayRect rect) | rect <- clayRects, clayCase rect == "treecheck"]
        maybeCheck <-
          withIOTimeout "Halay tree layout/check" $ do
            let actual = placedRects (randomTreeLayoutHalay randomTree)
            ok <- evaluate (sameRects expected actual)
            pure (actual, ok)
        pure $
          case maybeCheck of
            Left timeoutMessage ->
              counterexample
                ( timeoutMessage
                    <> "\nlayout:   "
                    <> show randomTree
                    <> "\ninput:    "
                    <> input
                    <> "expected: "
                    <> show expected
                )
                False
            Right (actual, ok) ->
              counterexample
                ( "layout:   "
                    <> show randomTree
                    <> "\ninput:    "
                    <> input
                    <> "expected: "
                    <> show expected
                    <> "\nactual:   "
                    <> show actual
                )
                ok

randomTreeLayoutHalay :: RandomTreeLayout -> Halay Identity Identity Placements
randomTreeLayoutHalay RandomTreeLayout {randomTreeRootConfig, randomTreeRootSize, randomTreeChildren} =
  namedLayout "root" $
    box (boxConfigFromRandom rootConfig)
      (snd (mapAccumL randomTreeNodeHalay 0 randomTreeChildren))
  where
    rootConfig =
      randomTreeRootConfig
        { randomBoxSizing =
            Sizing
              (Fixed (sizeWidth randomTreeRootSize))
              (Fixed (sizeHeight randomTreeRootSize))
        }

randomTreeNodeHalay :: Int -> RandomTreeNode -> (Int, Halay Identity Identity Placements)
randomTreeNodeHalay index node =
  case node of
    RandomTreeLeaf size sizing maybeAspect ->
      (index + 1, withAspect maybeAspect (namedLayout name (box (boxConfigFromRandom (leafBoxConfig sizing)) [fixed size mempty])))
    RandomTreeText config maybeAspect textContent ->
      (index + 1, withAspect maybeAspect (namedLayout name (box (boxConfigFromRandom config) [randomTreeTextHalay textContent])))
    RandomTreeBox config maybeAspect children ->
      let (nextIndex, childLayouts) = mapAccumL randomTreeNodeHalay (index + 1) children
       in (nextIndex, withAspect maybeAspect (namedLayout name (box (boxConfigFromRandom config) childLayouts)))
  where
    name = "n" <> show index

randomTreeTextHalay :: RandomTreeTextContent -> Halay Identity Identity Placements
randomTreeTextHalay RandomTreeTextContent {randomTreeTextWrapMode, randomTreeTextAlign, randomTreeTextFontSize, randomTreeTextLineHeight, randomTreeTextLineWordLengths} =
  text
    (testTextConfig randomTreeTextFontSize randomTreeTextLineHeight)
      { textWrapMode = randomTreeTextWrapMode
      , textAlign = randomTreeTextAlign
      , textPlaceLine = \_index _line _rect -> pure mempty
      }
    (linesText randomTreeTextLineWordLengths)

withAspect :: Maybe Double -> Halay Identity Identity Placements -> Halay Identity Identity Placements
withAspect Nothing layout = layout
withAspect (Just ratio) layout = aspectRatio ratio layout

namedLayout :: String -> Halay Identity Identity Placements -> Halay Identity Identity Placements
namedLayout name =
  decorate (\rect -> pure (Placements [(name, rect)]))

boxConfigFromRandom :: RandomBoxConfig -> BoxConfig
boxConfigFromRandom RandomBoxConfig {randomBoxDirection, randomBoxPadding, randomBoxGap, randomBoxAlignX, randomBoxAlignY, randomBoxSizing, randomBoxClipHorizontal, randomBoxClipVertical, randomBoxChildOffset} =
  defaultBox
    { boxDirection = randomBoxDirection
    , boxPadding = randomBoxPadding
    , boxGap = fromIntegral randomBoxGap
    , boxSizing = randomBoxSizing
    , boxClip = BoxClip randomBoxClipHorizontal randomBoxClipVertical randomBoxChildOffset
    , boxMainAlign =
        case randomBoxDirection of
          LeftToRight -> mainAlign randomBoxAlignX
          TopToBottom -> mainAlign randomBoxAlignY
    , boxCrossAlign =
        case randomBoxDirection of
          LeftToRight -> crossAlign randomBoxAlignY
          TopToBottom -> crossAlign randomBoxAlignX
    }

randomTreeOracleInput :: RandomTreeLayout -> String
randomTreeOracleInput RandomTreeLayout {randomTreeRootConfig, randomTreeRootSize, randomTreeChildren} =
  unwords ("treecheck" : (rootWords <> childWords))
    <> "\n"
  where
    rootConfig =
      randomTreeRootConfig
        { randomBoxSizing =
            Sizing
              (Fixed (sizeWidth randomTreeRootSize))
              (Fixed (sizeHeight randomTreeRootSize))
        }
    rootWords = treeNodeWords "root" TreeContainer (length randomTreeChildren) (Size 0 0) rootConfig Nothing
    (_nextIndex, childWords) = treeChildWords 0 randomTreeChildren

treeChildWords :: Int -> [RandomTreeNode] -> (Int, [String])
treeChildWords index nodes =
  let (nextIndex, wordsByNode) = mapAccumL treeNodeOracleWords index nodes
   in (nextIndex, concat wordsByNode)

treeNodeOracleWords :: Int -> RandomTreeNode -> (Int, [String])
treeNodeOracleWords index node =
  case node of
    RandomTreeLeaf size sizing aspect ->
      (index + 1, treeNodeWords name TreeIntrinsicLeaf 0 size (leafBoxConfig sizing) aspect)
    RandomTreeText config aspect textContent ->
      (index + 1, treeNodeWords name TreeTextLeaf 0 (Size 0 0) config aspect <> treeTextWords textContent)
    RandomTreeBox config aspect children ->
      let (nextIndex, childWords) = treeChildWords (index + 1) children
       in (nextIndex, treeNodeWords name TreeContainer (length children) (Size 0 0) config aspect <> childWords)
  where
    name = "n" <> show index

leafBoxConfig :: Sizing -> RandomBoxConfig
leafBoxConfig sizing =
  RandomBoxConfig
    { randomBoxDirection = LeftToRight
    , randomBoxPadding = Insets 0 0 0 0
    , randomBoxGap = 0
    , randomBoxAlignX = AlignStart
    , randomBoxAlignY = AlignStart
    , randomBoxSizing = sizing
    , randomBoxClipHorizontal = False
    , randomBoxClipVertical = False
    , randomBoxChildOffset = Point 0 0
    }

data TreeNodeKind
  = TreeIntrinsicLeaf
  | TreeTextLeaf
  | TreeContainer

treeNodeWords :: String -> TreeNodeKind -> Int -> Size -> RandomBoxConfig -> Maybe Double -> [String]
treeNodeWords name nodeKind childCount intrinsicSize RandomBoxConfig {randomBoxDirection, randomBoxPadding, randomBoxGap, randomBoxAlignX, randomBoxAlignY, randomBoxSizing, randomBoxClipHorizontal, randomBoxClipVertical, randomBoxChildOffset} maybeAspect =
  [ name
  , show childCount
  , show (sizeWidth intrinsicSize)
  , show (sizeHeight intrinsicSize)
  , show (directionValue randomBoxDirection)
  , show (round (insetLeft randomBoxPadding) :: Int)
  , show (round (insetRight randomBoxPadding) :: Int)
  , show (round (insetTop randomBoxPadding) :: Int)
  , show (round (insetBottom randomBoxPadding) :: Int)
  , show randomBoxGap
  , show (alignValue randomBoxAlignX)
  , show (alignValue randomBoxAlignY)
  , show (axisSizingValueType (sizingWidth randomBoxSizing))
  , show (axisSizingValueType (sizingHeight randomBoxSizing))
  , show (axisSizingValue (sizingWidth randomBoxSizing))
  , show (axisSizingValue (sizingHeight randomBoxSizing))
  , show (axisSizingMin (sizingWidth randomBoxSizing))
  , show (axisSizingMin (sizingHeight randomBoxSizing))
  , show (axisSizingMax (sizingWidth randomBoxSizing))
  , show (axisSizingMax (sizingHeight randomBoxSizing))
  , maybe "0" show maybeAspect
  , show (treeNodeKindValue nodeKind)
  , show (boolValue randomBoxClipHorizontal)
  , show (boolValue randomBoxClipVertical)
  , show (pointX randomBoxChildOffset)
  , show (pointY randomBoxChildOffset)
  ]

boolValue :: Bool -> Int
boolValue False = 0
boolValue True = 1

treeNodeKindValue :: TreeNodeKind -> Int
treeNodeKindValue TreeIntrinsicLeaf = 0
treeNodeKindValue TreeTextLeaf = 1
treeNodeKindValue TreeContainer = 2

treeTextWords :: RandomTreeTextContent -> [String]
treeTextWords RandomTreeTextContent {randomTreeTextWrapMode, randomTreeTextAlign, randomTreeTextFontSize, randomTreeTextLineHeight, randomTreeTextLineWordLengths} =
  [ show (textWrapModeValue randomTreeTextWrapMode)
  , show (textAlignValue randomTreeTextAlign)
  , show randomTreeTextFontSize
  , show (fromMaybe 0 randomTreeTextLineHeight)
  , show (length randomTreeTextLineWordLengths)
  ]
    <> concatMap lineWords randomTreeTextLineWordLengths
  where
    lineWords wordLengths =
      show (length wordLengths) : (show <$> wordLengths)

childNames :: [String]
childNames = ["a", "b", "c", "d"]

directionValue :: Direction -> Int
directionValue LeftToRight = 0
directionValue TopToBottom = 1

alignValue :: AlignChoice -> Int
alignValue AlignStart = 0
alignValue AlignCenter = 1
alignValue AlignEnd = 2

axisSizingValueType :: AxisSizing -> Int
axisSizingValueType sizing =
  case sizing of
    Fit {} -> 0
    Fixed {} -> 1
    Fill {} -> 2
    Percent {} -> 3

axisSizingValue :: AxisSizing -> Double
axisSizingValue sizing =
  case sizing of
    Fixed value -> value
    Percent value -> value
    _ -> 0

axisSizingMin :: AxisSizing -> Double
axisSizingMin sizing =
  case sizingBounds sizing of
    MinMax (Just value) _ -> value
    _ -> 0

axisSizingMax :: AxisSizing -> Double
axisSizingMax sizing =
  case sizingBounds sizing of
    MinMax _ (Just value) -> value
    _ -> 0

sizingBounds :: AxisSizing -> MinMax
sizingBounds (Fit minMax) = minMax
sizingBounds (Fill minMax) = minMax
sizingBounds (Fixed value) = MinMax (Just value) (Just value)
sizingBounds (Percent _) = unbounded

fixedAxisSize :: AxisSizing -> Maybe Double
fixedAxisSize (Fixed value) = Just value
fixedAxisSize _ = Nothing

named :: String -> Size -> Halay Identity Identity Placements
named name size =
  leaf (pure size) (\rect -> pure (Placements [(name, rect)]))

placedRectsWithRoot :: Halay Identity Identity Placements -> [(String, Rect)]
placedRectsWithRoot layout =
  ("root", Rect 0 0 (sizeWidth size) (sizeHeight size)) : rects
  where
    (size, Placements rects) = runIdentity (placeAt (Point 0 0) layout)

placedRects :: Halay Identity Identity Placements -> [(String, Rect)]
placedRects layout =
  rects
  where
    (_size, Placements rects) = runIdentity (placeAt (Point 0 0) layout)

placeAt :: (Monad measureM, Monoid placed) => Point -> Halay measureM measureM placed -> measureM (Size, placed)
placeAt point layout = do
  measured <- measureHalay layout
  placed <- placeMeasured measured (sizeRectAt point (measuredSize measured))
  pure (measuredSize measured, placed)
