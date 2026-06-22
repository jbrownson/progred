module Puri.Widgets.ScrollViewport
  ( ScrollPlacementTrace (..)
  , clampScrollOffset
  , scrollViewport
  , scrollWheelDelta
  , traceScrollPlacement
  ) where

import Control.Monad.IO.Class (MonadIO, liftIO)
import Data.IORef (newIORef, readIORef, writeIORef)
import Halay
import qualified Puri.Canvas as Canvas
import Puri.Handler

data ScrollPlacementTrace = ScrollPlacementTrace
  { scrollTraceLayoutRect :: Rect
  , scrollTraceViewportClip :: Rect
  , scrollTraceLayoutContentSize :: Size
  , scrollTraceRequestedOffset :: Point
  , scrollTraceAppliedOffset :: Point
  , scrollTraceChildOffset :: Point
  , scrollTraceChildRect :: Rect
  }
  deriving (Eq, Show)

scrollViewport
  :: (Canvas.Canvas renderM, Monad actionM)
  => renderM Point
  -> actionM Point
  -> (Point -> actionM ())
  -> Halay renderM renderM (Handler actionM)
  -> Halay renderM renderM (Handler actionM)
scrollViewport =
  scrollViewportImpl Nothing

traceScrollPlacement
  :: (Canvas.Canvas m, MonadIO m)
  => Rect
  -> Point
  -> Halay m m (Handler IO)
  -> m ScrollPlacementTrace
traceScrollPlacement viewport offset content = do
  childRectRef <- liftIO (newIORef (Rect 0 0 0 0))
  traceRef <- liftIO $
    newIORef
      ScrollPlacementTrace
        { scrollTraceLayoutRect = viewport
        , scrollTraceViewportClip = viewport
        , scrollTraceLayoutContentSize = Size 0 0
        , scrollTraceRequestedOffset = offset
        , scrollTraceAppliedOffset = Point 0 0
        , scrollTraceChildOffset = Point 0 0
        , scrollTraceChildRect = Rect 0 0 0 0
        }
  let tracked =
        decorate
          ( \placement ->
              liftIO (writeIORef childRectRef (placementRect placement))
                >> pure mempty
          )
          content
  let layout =
        scrollViewportImpl
          (Just (\trace -> liftIO (writeIORef traceRef trace)))
          (pure offset)
          (pure offset)
          (const $ pure ())
          tracked
  measured <- measureHalay layout
  placeMeasured measured (rootPlacement viewport) >>= \_handler -> do
    childRect <- liftIO (readIORef childRectRef)
    trace <- liftIO (readIORef traceRef)
    pure trace {scrollTraceChildRect = childRect}

scrollViewportImpl
  :: (Canvas.Canvas renderM, Monad actionM)
  => Maybe (ScrollPlacementTrace -> renderM ())
  -> renderM Point
  -> actionM Point
  -> (Point -> actionM ())
  -> Halay renderM renderM (Handler actionM)
  -> Halay renderM renderM (Handler actionM)
scrollViewportImpl tracePlacement getPlacementOffset getWheelOffset setOffset inner =
  container
    defaultBox
      { boxDirection = TopToBottom
      , boxSizing = Sizing (Fill unbounded) (Fill unbounded)
      -- Layout overflow zone: keep content-sized children on the main axis.
      , boxClip = BoxClip True True (Point 0 0)
      }
    placeScroll
    [inner]
  where
    placeScroll placement node placeKids = do
      offset <- getPlacementOffset
      let viewportClip = clipRect placement
      let contentSize = containerLaidOutChildSize node
      let applied = clampScrollOffset offset viewportClip contentSize
      let childOffset =
            Point
              { pointX = negate (pointX applied)
              , pointY = negate (pointY applied)
              }
      placedKids <-
        Canvas.withClip viewportClip (placeKids childOffset)
      recordTrace tracePlacement placement contentSize offset applied childOffset
      pure (scrollWheelHandler viewportClip getWheelOffset setOffset contentSize <> placedKids)

recordTrace
  :: Monad renderM
  => Maybe (ScrollPlacementTrace -> renderM ())
  -> Placement
  -> Size
  -> Point
  -> Point
  -> Point
  -> renderM ()
recordTrace Nothing _ _ _ _ _ =
  pure ()
recordTrace (Just trace) placement contentSize requested applied childOffset =
  trace
    ScrollPlacementTrace
      { scrollTraceLayoutRect = placementRect placement
      , scrollTraceViewportClip = clipRect placement
      , scrollTraceLayoutContentSize = contentSize
      , scrollTraceRequestedOffset = requested
      , scrollTraceAppliedOffset = applied
      , scrollTraceChildOffset = childOffset
      , scrollTraceChildRect = Rect 0 0 0 0
      }

clampScrollOffset :: Point -> Rect -> Size -> Point
clampScrollOffset offset viewport contentSize =
  Point
    { pointX = clamp 0 (maxScrollX viewport (sizeWidth contentSize)) (pointX offset)
    , pointY = clamp 0 (maxScrollY viewport (sizeHeight contentSize)) (pointY offset)
    }

maxScrollX :: Rect -> Double -> Double
maxScrollX viewport contentWidth =
  max 0 (contentWidth - width viewport)

maxScrollY :: Rect -> Double -> Double
maxScrollY viewport contentHeight =
  max 0 (contentHeight - height viewport)

scrollWheelHandler
  :: Monad actionM
  => Rect
  -> actionM Point
  -> (Point -> actionM ())
  -> Size
  -> Handler actionM
scrollWheelHandler viewportRect getOffset setOffset contentSize =
  onWheel $ \event@Wheel {wheelX, wheelY} ->
    if rectContains viewportRect wheelX wheelY
      then
        Just $ do
          offset <- getOffset
          setOffset
            ( clampScrollOffset
                (pointAdd offset (scrollWheelDelta event))
                viewportRect
                contentSize
            )
      else Nothing

scrollWheelDelta :: WheelEvent -> Point
scrollWheelDelta Wheel {wheelDeltaX, wheelDeltaY, wheelDeltaMode} =
  Point (signedDeltaX * factor) (signedDeltaY * factor)
  where
    factor
      | wheelDeltaMode == 0 = 1
      | otherwise = 16
    signedDeltaX
      | wheelDeltaMode == 0 = wheelDeltaX
      | otherwise = negate wheelDeltaX
    signedDeltaY
      | wheelDeltaMode == 0 = wheelDeltaY
      | otherwise = negate wheelDeltaY

clamp :: Double -> Double -> Double -> Double
clamp lo hi value =
  max lo (min hi value)

pointAdd :: Point -> Point -> Point
pointAdd Point {pointX = leftX, pointY = leftY} Point {pointX = rightX, pointY = rightY} =
  Point (leftX + rightX) (leftY + rightY)