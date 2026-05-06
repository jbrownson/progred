module Progred.View
  ( Attr
  , View
  , attr
  , class_
  , element
  , element_
  , id_
  , renderHtml
  , text
  ) where

data Attr = Attr String String
  deriving (Eq, Show)

data View
  = Element String [Attr] [View]
  | Text String
  deriving (Eq, Show)

attr :: String -> String -> Attr
attr = Attr

class_ :: String -> Attr
class_ = attr "class"

id_ :: String -> Attr
id_ = attr "id"

element :: String -> [Attr] -> [View] -> View
element = Element

element_ :: String -> [View] -> View
element_ name = element name []

text :: String -> View
text = Text

renderHtml :: View -> String
renderHtml view =
  case view of
    Text value -> escapeText value
    Element name attrs children ->
      if isVoidElement name
        then "<" <> name <> renderAttrs attrs <> ">"
        else
          "<" <> name <> renderAttrs attrs <> ">"
            <> concatMap renderHtml children
            <> "</" <> name <> ">"

isVoidElement :: String -> Bool
isVoidElement name =
  name `elem`
    [ "area"
    , "base"
    , "br"
    , "col"
    , "embed"
    , "hr"
    , "img"
    , "input"
    , "link"
    , "meta"
    , "param"
    , "source"
    , "track"
    , "wbr"
    ]

renderAttrs :: [Attr] -> String
renderAttrs = concatMap renderAttr

renderAttr :: Attr -> String
renderAttr (Attr name value) =
  " " <> name <> "=\"" <> escapeAttr value <> "\""

escapeText :: String -> String
escapeText =
  concatMap escapeChar

escapeAttr :: String -> String
escapeAttr =
  concatMap escapeAttrChar

escapeAttrChar :: Char -> String
escapeAttrChar char =
  case char of
    '"' -> "&quot;"
    _ -> escapeChar char

escapeChar :: Char -> String
escapeChar char =
  case char of
    '&' -> "&amp;"
    '<' -> "&lt;"
    '>' -> "&gt;"
    _ -> [char]
