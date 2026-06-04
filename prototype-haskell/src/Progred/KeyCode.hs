module Progred.KeyCode
  ( backspace
  , delete
  , down
  , end
  , enter
  , home
  , left
  , right
  , shiftLeft
  , shiftRight
  , shiftTab
  , space
  , tab
  , up
  , withShift
  ) where

import Data.Word (Word32)

backspace, tab, enter, space, home, end, left, up, right, down, delete :: Word32
backspace = 8
tab = 9
enter = 13
space = 32
home = 36
end = 35
left = 37
up = 38
right = 39
down = 40
delete = 46

shiftLeft, shiftRight, shiftTab :: Word32
shiftLeft = withShift left
shiftRight = withShift right
shiftTab = withShift tab

withShift :: Word32 -> Word32
withShift keyCode =
  keyCode + shiftOffset

shiftOffset :: Word32
shiftOffset = 1000
