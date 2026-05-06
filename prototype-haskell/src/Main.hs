module Main (main, hello) where

import Data.List (sortOn)
import Data.Word (Word32)
import Progred.Gid (Gid, edgeList, nidEdges, runGid)
import Progred.MapGid (MapGid, emptyMapGid, mapGid, setEdge)
import Progred.Platform (logClick, setRootHtml)
import Progred.View (View, attr, class_, element, renderHtml, text)

type DemoId = String

rootId :: DemoId
rootId = "root"

demoGid :: MapGid DemoId
demoGid =
  foldr (\(source, label, target) -> setEdge source label target) emptyMapGid
    [ ("child-a", "next", "child-b")
    , ("child-b", "back", "root")
    , ("root", "left", "child-a")
    , ("root", "right", "child-b")
    , ("root", "empty", "leaf")
    ]

appView :: Maybe Word32 -> View
appView _maybeClick =
  element "div" [class_ "scrollparent"]
    [ element "main" [class_ "doc"]
        [ renderNode 0 [] (mapGid demoGid) rootId ]
    ]

renderNode :: Int -> [DemoId] -> Gid DemoId -> DemoId -> View
renderNode depth seen gid source
  | source `elem` seen =
      element "span" [class_ "uneditable"] [text ("cycle " <> source)]
  | otherwise =
      case runGid gid source of
        Nothing ->
          element "span" [class_ "unmatching"] [text source]
        Just nid ->
          element "span" [class_ "descend"] $
            text source : map renderEdge (sortOn fst (edgeList (nidEdges nid)))
  where
    renderEdge (label, target) =
      element "span" []
        [ element "br" [] []
        , indent (depth + 1)
        , element "span" [class_ "edgefield"]
            [ element "span" [class_ "edgeLabel"] [text (label <> " →")]
            , text " "
            , renderNode (depth + 1) (source : seen) gid target
            ]
        ]

indent :: Int -> View
indent depth =
  element "span"
    [ attr "style" ("width: " <> show (16 * depth) <> "px; display: inline-block") ]
    []

renderApp :: Maybe Word32 -> IO ()
renderApp maybeClick =
  setRootHtml (renderHtml (appView maybeClick))

main :: IO ()
main = renderApp Nothing

hello :: Word32 -> IO ()
hello n = do
  logClick n
  renderApp (Just n)
