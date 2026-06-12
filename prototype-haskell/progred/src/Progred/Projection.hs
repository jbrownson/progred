module Progred.Projection
  ( Cursor (..)
  , Env (..)
  , Layer
  , projectDocument
  , stepCursor
  ) where

import Data.Foldable (asum)
import Data.Maybe (fromMaybe)
import Halay
import Progred.Document
import Progred.Editor
import Progred.Graph
import Puri.Handler

-- A projection is a stack of layers. Each layer may decline a spot; the
-- total fallback at the bottom may not. Layers render children through
-- envProject so every child is offered to the whole stack again.
data Env actionM renderM = Env
  { envDocument :: Document
  , envEdit :: (Editor -> Editor) -> actionM ()
  , envProject :: Cursor -> Halay renderM (Handler actionM)
  }

type Layer actionM renderM = Env actionM renderM -> Cursor -> Maybe (Halay renderM (Handler actionM))

-- A spot in the document: the label path that leads there and the focus
-- remainder peeled while descending. Spots need not resolve to anything
-- in the graph.
data Cursor = Cursor
  { cursorPath :: [UUID]
  , cursorFocus :: Maybe Focus
  }

projectDocument
  :: [Layer actionM renderM]
  -> (Env actionM renderM -> Cursor -> Halay renderM (Handler actionM))
  -> Document
  -> ((Editor -> Editor) -> actionM ())
  -> Maybe Focus
  -> Halay renderM (Handler actionM)
projectDocument layers fallback document edit focus =
  project (Cursor [] focus)
  where
    env = Env {envDocument = document, envEdit = edit, envProject = project}
    project cursor =
      fromMaybe (fallback env cursor) (asum [layer env cursor | layer <- layers])

stepCursor :: UUID -> Cursor -> Cursor
stepCursor label cursor =
  Cursor
    { cursorPath = cursorPath cursor <> [label]
    , cursorFocus =
        case cursorFocus cursor of
          Just (Focus (step : rest) view) | step == label -> Just (Focus rest view)
          _ -> Nothing
    }
