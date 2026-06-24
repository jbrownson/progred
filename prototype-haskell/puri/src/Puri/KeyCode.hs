module Puri.KeyCode
  ( backspace
  , comma
  , delete
  , down
  , end
  , enter
  , escape
  , home
  , left
  , right
  , space
  , tab
  , up
  ) where

import Data.Word (Word32)

backspace, tab, comma, enter, escape, space, home, end, left, up, right, down, delete :: Word32
backspace = 8
tab = 9
comma = 188
enter = 13
escape = 27
space = 32
home = 36
end = 35
left = 37
up = 38
right = 39
down = 40
delete = 46
