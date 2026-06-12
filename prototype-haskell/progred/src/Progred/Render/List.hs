module Progred.Render.List
  ( listLayer
  ) where

import Data.List (intersperse)
import qualified Data.Map.Strict as Map
import Halay
import Progred.Builtins
import Progred.Document
import Progred.Graph
import Progred.Projection
import Progred.Render.Raw (textPlay)
import qualified Puri.Canvas as Canvas

-- Projects cons chains as bracketed lists. Declines anything that isn't
-- a well-formed chain (cells with exactly head and tail, ending at nil,
-- acyclic) so the fallback keeps every malformed detail visible.
listLayer :: Canvas.Canvas renderM => Layer actionM renderM
listLayer env cursor =
  render <$> elements [] cursor
  where
    document = envDocument env
    elements seen spot = do
      (_, value) <- walkPath document (cursorPath spot)
      case value of
        VRef node
          | node == nilNode -> Just []
          | node `elem` seen -> Nothing
          | otherwise -> do
              edges <- Map.lookup node (documentGraph document)
              if Map.size edges == 2 && Map.member headLabel edges && Map.member tailLabel edges
                then (stepCursor headLabel spot :) <$> elements (node : seen) (stepCursor tailLabel spot)
                else Nothing
        _ -> Nothing
    render [] = textPlay listColor "[]"
    render spots =
      rowWithGap
        listGap
        ([textPlay listColor "["] <> intersperse (textPlay listColor ",") (envProject env <$> spots) <> [textPlay listColor "]"])

listColor :: String
listColor = "#68707c"

listGap :: Double
listGap = 6
