module Puri.Lens
  ( Lens (..)
  , unitLens
  ) where

data Lens whole part = Lens
  { lensGet :: whole -> part
  , lensSet :: part -> whole -> whole
  }

unitLens :: Lens whole ()
unitLens =
  Lens
    { lensGet = const ()
    , lensSet = \() whole -> whole
    }
