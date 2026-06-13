module Progred.Projection
  ( Cursor (..)
  , Env (..)
  , PartialProjection (..)
  , Projection
  , over
  , projectDocument
  , stepCursor
  ) where

import Control.Applicative ((<|>))
import Data.Maybe (fromMaybe)
import Halay
import Progred.Document
import Progred.Editor
import Progred.Graph
import Puri.Handler

-- A Projection maps a spot to a layout and always succeeds. A
-- PartialProjection may decline; its Monoid composes by first
-- acceptance, and `over` lays one across a Projection to fill the
-- declined spots. Children recurse through envProject, so every child
-- is offered the whole composition again.
type Projection actionM renderM = Env actionM renderM -> Cursor -> Halay renderM (Handler actionM)

newtype PartialProjection actionM renderM = PartialProjection
  { tryProject :: Env actionM renderM -> Cursor -> Maybe (Halay renderM (Handler actionM))
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
  { envDocument :: Document
  , envEdit :: (Editor -> Editor) -> actionM ()
  , envProject :: Cursor -> Halay renderM (Handler actionM)
  }

-- A spot in the document: the label path that leads there and the focus
-- remainder peeled while descending. Spots need not resolve to anything
-- in the graph.
data Cursor = Cursor
  { cursorPath :: [UUID]
  , cursorFocus :: Maybe Focus
  }

projectDocument
  :: Projection actionM renderM
  -> Document
  -> ((Editor -> Editor) -> actionM ())
  -> Maybe Focus
  -> Halay renderM (Handler actionM)
projectDocument total document edit focus =
  apply (Cursor [] focus)
  where
    env = Env {envDocument = document, envEdit = edit, envProject = apply}
    apply = total env

stepCursor :: UUID -> Cursor -> Cursor
stepCursor label cursor =
  Cursor
    { cursorPath = cursorPath cursor <> [label]
    , cursorFocus =
        case cursorFocus cursor of
          Just (Focus (step : rest) view) | step == label -> Just (Focus rest view)
          _ -> Nothing
    }
