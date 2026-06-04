module Progred.Widget.Interpreter
  ( WidgetM
  , WidgetEnv (..)
  , runWidgetFrame
  ) where

import Progred.Frame
import Progred.Widget

data WidgetEnv state appM = WidgetEnv
  { widgetEnvPutState :: state -> appM ()
  , widgetEnvFocusSelf :: appM ()
  }

newtype WidgetM state appM a = WidgetM
  { runWidgetM :: WidgetEnv state appM -> appM a
  }

instance Functor appM => Functor (WidgetM state appM) where
  fmap f action =
    WidgetM $ \env -> fmap f (runWidgetM action env)

instance Applicative appM => Applicative (WidgetM state appM) where
  pure value =
    WidgetM $ \_env -> pure value
  function <*> argument =
    WidgetM $ \env ->
      runWidgetM function env <*> runWidgetM argument env

instance Monad appM => Monad (WidgetM state appM) where
  action >>= f =
    WidgetM $ \env -> do
      value <- runWidgetM action env
      runWidgetM (f value) env

instance WidgetActions state appM (WidgetM state appM) where
  putState state =
    WidgetM $ \env -> widgetEnvPutState env state
  focusSelf =
    WidgetM widgetEnvFocusSelf
  liftApp action =
    WidgetM $ const action

runWidgetFrame :: WidgetEnv state appM -> Frame (WidgetM state appM) -> Frame appM
runWidgetFrame env frame =
  Frame
    { draws = draws frame
    , pointerHandlers = fmap runPointerHandler (pointerHandlers frame)
    , keyHandlers = fmap runKeyHandler (keyHandlers frame)
    }
  where
    runPointerHandler handler event =
      fmap (`runWidgetM` env) (handler event)
    runKeyHandler handler event =
      fmap (`runWidgetM` env) (handler event)
