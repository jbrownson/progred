module Progred.Render.List
  ( listProjection
  ) where

import Data.List (intersperse)
import qualified Data.Map.Strict as Map
import Halay
import Progred.Builtins
import Progred.Graph
import Progred.GraphContext
import Progred.Projection
import Progred.Render.Raw (inlineRowWithGap, textPlay)
import qualified Puri.Canvas as Canvas
import Puri.Handler

-- Projects cons chains as bracketed lists. Declines anything that isn't
-- a well-formed chain (cells with exactly head and tail, ending at nil,
-- acyclic) so the fallback keeps every malformed detail visible.
listProjection :: Canvas.Canvas renderM => PartialProjection actionM renderM
listProjection =
  PartialProjection projectList

projectList :: Canvas.Canvas renderM => Env actionM renderM -> Cursor -> Maybe (Halay renderM renderM (Handler actionM))
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
              if Map.size edges == 2 && Map.member headLabel edges && Map.member tailLabel edges
                then (descendCursor headLabel spot :) <$> elements (node : seen) (descendCursor tailLabel spot)
                else Nothing
        _ -> Nothing
    render [] = textPlay listColor "[]"
    render spots =
      inlineRowWithGap
        listGap
        ([textPlay listColor "["] <> intersperse (textPlay listColor ",") (envProject env <$> spots) <> [textPlay listColor "]"])

listColor :: String
listColor = "#68707c"

listGap :: Double
listGap = 6
