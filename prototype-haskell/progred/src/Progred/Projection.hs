module Progred.Projection
  ( Cursor (..)
  , Env (..)
  , Projection (..)
  , TotalProjection
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

-- A projection may decline a spot; composition takes the first that
-- accepts. Children recurse through envProject, so every child is
-- offered the whole composition again. A complete composition needs a
-- total projection at the bottom, which only `over` can provide.
newtype Projection actionM renderM = Projection
  { project :: Env actionM renderM -> Cursor -> Maybe (Halay renderM (Handler actionM))
  }

type TotalProjection actionM renderM = Env actionM renderM -> Cursor -> Halay renderM (Handler actionM)

instance Semigroup (Projection actionM renderM) where
  earlier <> later =
    Projection (\env cursor -> project earlier env cursor <|> project later env cursor)

instance Monoid (Projection actionM renderM) where
  mempty = Projection (\_ _ -> Nothing)

over :: Projection actionM renderM -> TotalProjection actionM renderM -> TotalProjection actionM renderM
over projection total env cursor =
  fromMaybe (total env cursor) (project projection env cursor)

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
  :: TotalProjection actionM renderM
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
