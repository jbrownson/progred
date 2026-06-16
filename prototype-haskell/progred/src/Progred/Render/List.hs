module Progred.Render.List
  ( listProjection
  ) where

import qualified Data.Map.Strict as Map
import Halay
import Progred.Builtins
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Progred.Projection
import Progred.Render.Raw (inlineRowWithGap, textPlay)
import qualified Puri.Canvas as Canvas
import Puri.Halay (lineEdit)
import Puri.Handler
import qualified Puri.KeyCode as KeyCode
import Puri.Widgets (LineEditInteraction (..), LineEditSelection (..), LineStyle (..))
import Puri.Widgets.Frame

-- Projects explicit cons chains as bracketed lists. Cons cells opt in with
-- isa -> listConsNode; nilNode is the shared empty-list terminator.
-- Malformed chains decline so the fallback keeps the details visible.
listProjection :: (Canvas.Canvas renderM, Monad actionM) => PartialProjection actionM renderM
listProjection =
  PartialProjection projectList

projectList :: (Canvas.Canvas renderM, Monad actionM) => Env actionM renderM -> Cursor -> Maybe (Halay renderM renderM (Handler actionM))
projectList env cursor =
  render <$> elements [] cursor
  where
    context = envContext env
    elements seen spot = do
      resolved <- resolveCursor env spot
      case resolvedValue resolved of
        VRef node
          | node == nilNode -> Just []
          | node `elem` seen -> Nothing
          | otherwise -> do
              edges <- lookupNode context node
              if isListCons edges
                then
                  let after = descendCursor tailLabel spot
                   in (ListItem (descendCursor headLabel spot) after :) <$> elements (node : seen) after
                else Nothing
        _ -> Nothing
    render items =
      listActions env cursor $
        padding listPadding $
          inlineRowWithGap
            0
            (renderItems cursor items)
    renderItems before [] =
      emptyList env before
    renderItems before (item : rest) =
      [openBracket]
        <> startEntry
        <> renderActualItems item rest
      where
        startEntry =
          case activePending before of
            Just pending -> [pendingInsert env before pending, plainSeparator]
            Nothing -> [insertionAnchor env before]
    renderActualItems item rest =
      [projectItem item] <> renderAfter item rest
    renderAfter item [] =
      case activePending (listItemAfter item) of
        Just pending -> [plainSeparator, pendingInsert env (listItemAfter item) pending, closeBracket]
        Nothing -> [insertionAnchor env (listItemAfter item), closeBracket]
    renderAfter item (next : rest) =
      case activePending (listItemAfter item) of
        Just pending ->
          [plainSeparator, pendingInsert env (listItemAfter item) pending, plainSeparator]
            <> renderActualItems next rest
        Nothing ->
          [plainSeparator, insertionAnchor env (listItemAfter item)] <> renderActualItems next rest
    projectItem item =
      listItemActions env item (focusableEdge env (listItemHead item) (envProject env (listItemHead item)))

data ListItem = ListItem
  { listItemHead :: Cursor
  , listItemAfter :: Cursor
  }

listActions :: Applicative renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
listActions env cursor child =
  case cursorFocus cursor of
    Just focus | null (focusPath focus) && focusPendingEdit (focusState focus) == Nothing ->
      decorate (const (pure (onInsert (envEdit env (focusPending (listPendingPath (cursorPath cursor)) "" emptySelection))))) child
    _ -> child

listItemActions :: Applicative renderM => Env actionM renderM -> ListItem -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
listItemActions env item child =
  case cursorFocus (listItemHead item) of
    Just focus | null (focusPath focus) ->
      decorate
        ( const $
            pure $
              onDelete (envEdit env (spliceListItem (cursorPath (listItemHead item))))
                <> onInsert (envEdit env (focusPending (listPendingPath (cursorPath (listItemAfter item))) "" emptySelection))
        )
        child
    _ -> child

emptyList :: (Canvas.Canvas renderM, Monad actionM) => Env actionM renderM -> Cursor -> [Halay renderM renderM (Handler actionM)]
emptyList env cursor =
  case activePending cursor of
    Just pending -> [openBracket, pendingInsert env cursor pending, closeBracket]
    Nothing -> [openBracket, insertionAnchor env cursor, closeBracket]

openBracket :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
openBracket =
  bracket LeftBracket

closeBracket :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
closeBracket =
  bracket RightBracket

data BracketSide = LeftBracket | RightBracket

bracket :: Canvas.Canvas renderM => BracketSide -> Halay renderM renderM (Handler actionM)
bracket side =
  leafWithSizing bracketSizing (pure (Size bracketWidth bracketMinHeight)) draw
  where
    draw rect =
      mempty <$ drawBracket side rect

drawBracket :: Canvas.Canvas renderM => BracketSide -> Rect -> renderM ()
drawBracket side Rect {x, y, width, height} = do
  Canvas.fillRect (Rect verticalX y bracketStroke height) listColor
  Canvas.fillRect (Rect horizontalX y bracketTick bracketStroke) listColor
  Canvas.fillRect (Rect horizontalX (y + height - bracketStroke) bracketTick bracketStroke) listColor
  where
    verticalX =
      case side of
        LeftBracket -> x
        RightBracket -> x + width - bracketStroke
    horizontalX =
      case side of
        LeftBracket -> x
        RightBracket -> x + width - bracketTick

activePending :: Cursor -> Maybe PendingEdit
activePending cursor =
  case cursorFocus cursor of
    Just focus | focusPath focus == [listBeforeLabel] -> focusPendingEdit (focusState focus)
    _ -> Nothing

insertionAnchor :: Applicative renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM)
insertionAnchor env cursor =
  leafWithSizing anchorSizing (pure (Size 0 0)) draw
  where
    draw rect =
      pure (insertionAnchorHandler env cursor rect)

insertionAnchorHandler :: Env actionM renderM -> Cursor -> Rect -> Handler actionM
insertionAnchorHandler env cursor rect =
  onPointerCapture $ \event ->
    case event of
      PointerDown {pointerX, pointerY}
        | rectContains (expandHorizontal insertOverlap rect) pointerX pointerY ->
            Just (envEdit env (focusPending (listPendingPath path) "" emptySelection))
      _ -> Nothing
  where
    path = cursorPath cursor

expandHorizontal :: Double -> Rect -> Rect
expandHorizontal amount Rect {x, y, width, height} =
  Rect (x - amount) y (width + 2 * amount) height

pendingInsert :: (Canvas.Canvas renderM, Monad actionM) => Env actionM renderM -> Cursor -> PendingEdit -> Halay renderM renderM (Handler actionM)
pendingInsert env cursor pending =
  framed pendingFrame $
    decorate submitKeys $
      lineEdit pendingLineStyle currentText interaction
  where
    linkPath = cursorPath cursor
    pendingPath = listPendingPath linkPath
    currentText = pendingEditText pending
    selection = pendingEditSelection pending
    interaction =
      LineEditFocused
        selection
        (\newText newSelection -> envEdit env (focusPending pendingPath newText newSelection))
        (envEdit env (cancelPending pendingPath))
    submitKeys _rect =
      pure $
        onKey $ \event ->
          case event of
            KeyCode modifiers code
              | code == KeyCode.enter && not (hasModifier modifiers) ->
                  Just commit
              | code == KeyCode.escape ->
                  Just (envEdit env (cancelPending pendingPath))
            _ -> Nothing
    commit = do
      cell <- envFreshUUID env
      envEdit env (insertListString linkPath cell currentText (selectionAtEnd currentText))

isListCons :: Edges -> Bool
isListCons edges =
  Map.lookup isaLabel edges == Just (VRef listConsNode)
    && Map.member headLabel edges
    && Map.member tailLabel edges

listPendingPath :: [UUID] -> [UUID]
listPendingPath path =
  path <> [listBeforeLabel]

hasModifier :: KeyModifiers -> Bool
hasModifier modifiers =
  keyShift modifiers || keyAlt modifiers || keyCtrl modifiers || keyMeta modifiers

selectionAtEnd :: String -> LineEditSelection
selectionAtEnd string =
  LineEditSelection (length string) (length string) False

emptySelection :: LineEditSelection
emptySelection =
  LineEditSelection 0 0 False

listColor :: String
listColor = "#68707c"

plainSeparator :: Canvas.Canvas renderM => Halay renderM renderM (Handler actionM)
plainSeparator =
  textPlay listColor ", "

insertOverlap :: Double
insertOverlap = 4

anchorSizing :: Sizing
anchorSizing =
  Sizing (Fixed 0) (Fill unbounded)

bracketSizing :: Sizing
bracketSizing =
  Sizing (Fixed bracketWidth) (Fill unbounded)

bracketWidth :: Double
bracketWidth = 7

bracketMinHeight :: Double
bracketMinHeight = 20

bracketStroke :: Double
bracketStroke = 1.5

bracketTick :: Double
bracketTick = 6

listPadding :: Insets
listPadding =
  Insets 2 3 2 3

pendingFrame :: Frame
pendingFrame =
  Frame
    { framePadding = Insets 0 0 0 0
    , frameInsets = Insets 0 0 0 0
    , frameBackground = Just "#fff9e8"
    , frameColor = "#0a84ff"
    }

pendingLineStyle :: LineStyle
pendingLineStyle =
  LineStyle
    { lineVerticalPadding = 2
    , linePadding = 5
    , lineMinWidth = 32
    , lineTextColor = "#20242a"
    , lineCaretColor = "#0a84ff"
    , lineSelectionColor = "#cfe3ff"
    }
