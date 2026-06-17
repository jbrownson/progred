module Progred.PathTrie
  ( PathTrie
  , deletePath
  , empty
  , insertPath
  , lookupPath
  ) where

import qualified Data.Map.Strict as Map
import Progred.Graph (UUID)

data PathTrie a = PathTrie
  { pathTrieValue :: Maybe a
  , pathTrieChildren :: Map.Map UUID (PathTrie a)
  }
  deriving (Eq, Show)

empty :: PathTrie a
empty =
  PathTrie
    { pathTrieValue = Nothing
    , pathTrieChildren = Map.empty
    }

lookupPath :: [UUID] -> PathTrie a -> Maybe a
lookupPath path trie =
  case path of
    [] -> pathTrieValue trie
    label : rest -> do
      child <- Map.lookup label (pathTrieChildren trie)
      lookupPath rest child

insertPath :: [UUID] -> a -> PathTrie a -> PathTrie a
insertPath path value trie =
  case path of
    [] -> trie {pathTrieValue = Just value}
    label : rest ->
      trie
        { pathTrieChildren =
            Map.alter (Just . insertPath rest value . maybe empty id) label (pathTrieChildren trie)
        }

deletePath :: [UUID] -> PathTrie a -> PathTrie a
deletePath path trie =
  case path of
    [] -> trie {pathTrieValue = Nothing}
    label : rest ->
      trie
        { pathTrieChildren =
            Map.update (prune . deletePath rest) label (pathTrieChildren trie)
        }

prune :: PathTrie a -> Maybe (PathTrie a)
prune trie
  | hasNoValue && Map.null (pathTrieChildren trie) = Nothing
  | otherwise = Just trie
  where
    hasNoValue =
      case pathTrieValue trie of
        Nothing -> True
        Just _ -> False
