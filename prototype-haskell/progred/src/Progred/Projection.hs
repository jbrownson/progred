module Progred.Projection
  ( Cursor (..)
  , Env (..)
  , PartialProjection (..)
  , Projection
  , ResolvedCursor (..)
  , descend
  , descendCursor
  , over
  , projectContext
  , projectDocument
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

data Env actionM renderM = Env
  { envContext :: GraphContext
  , envEdit :: (Editor -> Editor) -> actionM ()
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
  -> Maybe Focus
  -> Halay renderM renderM (Handler actionM)
projectDocument total document edit focus =
  projectContext total (documentContext document []) edit focus

projectContext
  :: Projection actionM renderM
  -> GraphContext
  -> ((Editor -> Editor) -> actionM ())
  -> Maybe Focus
  -> Halay renderM renderM (Handler actionM)
projectContext total context edit focus =
  apply (Cursor [] focus)
  where
    env = Env {envContext = context, envEdit = edit, envProject = apply}
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

stepFocus :: UUID -> Focus -> Maybe Focus
stepFocus label focus =
  case focusPath focus of
    step : rest | step == label -> Just focus {focusPath = rest}
    _ -> Nothing
