module Progred.Projection
  ( Cursor (..)
  , Env (..)
  , PartialProjection (..)
  , Projection
  , ResolvedCursor (..)
  , SecondaryHighlight (..)
  , descend
  , descendCursor
  , focusableEdge
  , focusableSpot
  , over
  , projectContext
  , projectDocument
  , projectEditor
  , resolveCursor
  ) where

import Control.Applicative ((<|>))
import Data.Maybe (fromMaybe)
import Halay
import Progred.Document
import Progred.Editor
import Progred.Graph
import Progred.GraphContext
import Puri.Handler

-- A Projection maps a spot to a layout and always succeeds. A
-- PartialProjection may decline; its Monoid composes by first
-- acceptance, and `over` lays one across a Projection to fill the
-- declined spots. Children recurse through envProject, so every child
-- is offered the whole composition again.
type Projection actionM renderM = Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM)

newtype PartialProjection actionM renderM = PartialProjection
  { tryProject :: Env actionM renderM -> Cursor -> Maybe (Halay renderM renderM (Handler actionM))
  }

instance Semigroup (PartialProjection actionM renderM) where
  earlier <> later =
    PartialProjection (\env cursor -> tryProject earlier env cursor <|> tryProject later env cursor)

instance Monoid (PartialProjection actionM renderM) where
  mempty = PartialProjection (\_ _ -> Nothing)

over :: PartialProjection actionM renderM -> Projection actionM renderM -> Projection actionM renderM
over partial total env cursor =
  fromMaybe (total env cursor) (tryProject partial env cursor)

data SecondaryHighlight
  = SecondaryNode UUID
  | SecondarySpot [UUID]
  | SecondaryScalar ScalarKey
  deriving (Eq, Show)

data Env actionM renderM = Env
  { envContext :: GraphContext
  , envEdit :: (Editor -> Editor) -> actionM ()
  , envFreshUUID :: actionM UUID
  , envCollapseState :: [UUID] -> Maybe Bool
  , envSecondaryHighlight :: Maybe SecondaryHighlight
  , envProject :: Cursor -> Halay renderM renderM (Handler actionM)
  }

-- A spot in the document: the label path that leads there and the focus
-- remainder peeled while descending. Spots need not resolve to anything
-- in the graph.
data Cursor = Cursor
  { cursorPath :: [UUID]
  , cursorFocus :: Maybe Focus
  }

data ResolvedCursor = ResolvedCursor
  { resolvedCursor :: Cursor
  , resolvedNodes :: [UUID]
  , resolvedValue :: Value
  }

projectDocument
  :: Projection actionM renderM
  -> Document
  -> ((Editor -> Editor) -> actionM ())
  -> actionM UUID
  -> Maybe Focus
  -> Halay renderM renderM (Handler actionM)
projectDocument total document edit fresh focus =
  projectContext total (documentContext document []) edit fresh focus

projectEditor
  :: Projection actionM renderM
  -> Editor
  -> ((Editor -> Editor) -> actionM ())
  -> actionM UUID
  -> Maybe SecondaryHighlight
  -> Halay renderM renderM (Handler actionM)
projectEditor total editor edit fresh secondary =
  projectContextWith total (documentContext (editorDocument editor) []) edit fresh (editorFocus editor) (`collapseState` editor) secondary

projectContext
  :: Projection actionM renderM
  -> GraphContext
  -> ((Editor -> Editor) -> actionM ())
  -> actionM UUID
  -> Maybe Focus
  -> Halay renderM renderM (Handler actionM)
projectContext total context edit fresh focus =
  projectContextWith total context edit fresh focus (const Nothing) Nothing

projectContextWith
  :: Projection actionM renderM
  -> GraphContext
  -> ((Editor -> Editor) -> actionM ())
  -> actionM UUID
  -> Maybe Focus
  -> ([UUID] -> Maybe Bool)
  -> Maybe SecondaryHighlight
  -> Halay renderM renderM (Handler actionM)
projectContextWith total context edit fresh focus pathCollapseState secondary =
  apply (Cursor [] focus)
  where
    env =
      Env
        { envContext = context
        , envEdit = edit
        , envFreshUUID = fresh
        , envCollapseState = pathCollapseState
        , envSecondaryHighlight = secondary
        , envProject = apply
        }
    apply = total env

resolveCursor :: Env actionM renderM -> Cursor -> Maybe ResolvedCursor
resolveCursor env cursor = do
  PathWalk {walkedNodes = nodes, walkedValue = value} <- walkPath (envContext env) (cursorPath cursor)
  pure
    ResolvedCursor
      { resolvedCursor = cursor
      , resolvedNodes = nodes
      , resolvedValue = value
      }

descend :: Env actionM renderM -> Cursor -> UUID -> Halay renderM renderM (Handler actionM)
descend env cursor label =
  envProject env (descendCursor label cursor)

descendCursor :: UUID -> Cursor -> Cursor
descendCursor label cursor =
  Cursor
    { cursorPath = cursorPath cursor <> [label]
    , cursorFocus = stepFocus label =<< cursorFocus cursor
    }

focusableEdge :: Applicative renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
focusableEdge env cursor child
  | null path = child
  | otherwise = focusableSpot env cursor child
  where
    path = cursorPath cursor

focusableSpot :: Applicative renderM => Env actionM renderM -> Cursor -> Halay renderM renderM (Handler actionM) -> Halay renderM renderM (Handler actionM)
focusableSpot env cursor child =
  decorate place child
  where
    path = cursorPath cursor
    place placement =
      let rect = clipRect placement
       in pure $
            onPointer $ \event ->
              case event of
                PointerDown {pointerX, pointerY}
                  | rectContains rect pointerX pointerY ->
                      Just (envEdit env (focusSpot path))
                _ -> Nothing

stepFocus :: UUID -> Focus -> Maybe Focus
stepFocus label focus =
  case focusPath focus of
    step : rest | step == label -> Just focus {focusPath = rest}
    _ -> Nothing
