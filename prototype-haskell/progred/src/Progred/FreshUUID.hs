module Progred.FreshUUID
  ( MonadFreshUUID (..)
  , freshUUIDIO
  , seededUUIDs
  ) where

import Control.Monad.Trans.Class (lift)
import Control.Monad.Trans.State.Strict (StateT)
import Data.UUID.Types (UUID)
import qualified Data.UUID.Types as UUID
import Data.Word (Word32)
import System.Random (mkStdGen, randomIO, randoms)

class Monad m => MonadFreshUUID m where
  freshUUID :: m UUID

instance MonadFreshUUID IO where
  freshUUID = freshUUIDIO

instance (MonadFreshUUID m) => MonadFreshUUID (StateT s m) where
  freshUUID = lift freshUUID

freshUUIDIO :: IO UUID
freshUUIDIO = do
  word0 <- randomIO
  word1 <- randomIO
  word2 <- randomIO
  word3 <- randomIO
  pure (UUID.fromWords word0 word1 word2 word3)

seededUUIDs :: Int -> [UUID]
seededUUIDs seed = wordsToUUIDs (randoms (mkStdGen seed))

wordsToUUIDs :: [Word32] -> [UUID]
wordsToUUIDs (word0 : word1 : word2 : word3 : rest) =
  UUID.fromWords word0 word1 word2 word3 : wordsToUUIDs rest
wordsToUUIDs _ = []