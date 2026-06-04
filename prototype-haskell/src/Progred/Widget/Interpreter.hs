module Progred.Widget.Interpreter
  ( WidgetAction
  , WidgetEnv (..)
  , runWidget
  ) where

import Progred.Frame
import Progred.Widget

data WidgetEnv state actionM = WidgetEnv
  { widgetEnvPutState :: state -> actionM ()
  , widgetEnvFocusSelf :: actionM ()
  }

newtype WidgetAction state actionM a = WidgetAction
  { runWidgetAction :: WidgetEnv state actionM -> actionM a
  }

instance Functor actionM => Functor (WidgetAction state actionM) where
  fmap f action =
    WidgetAction $ \env -> fmap f (runWidgetAction action env)

instance Applicative actionM => Applicative (WidgetAction state actionM) where
  pure value =
    WidgetAction $ \_env -> pure value
  function <*> argument =
    WidgetAction $ \env ->
      runWidgetAction function env <*> runWidgetAction argument env

instance Monad actionM => Monad (WidgetAction state actionM) where
  action >>= f =
    WidgetAction $ \env -> do
      value <- runWidgetAction action env
      runWidgetAction (f value) env

instance WidgetActions state actionM (WidgetAction state actionM) where
  putState state =
    WidgetAction $ \env -> widgetEnvPutState env state
  focusSelf =
    WidgetAction widgetEnvFocusSelf
  liftAction action =
    WidgetAction $ const action

runWidget :: WidgetEnv state actionM -> Frame (WidgetAction state actionM) -> Frame actionM
runWidget env frame =
  Frame
    { draws = draws frame
    , pointerHandlers = fmap runPointerHandler (pointerHandlers frame)
    , keyHandlers = fmap runKeyHandler (keyHandlers frame)
    }
  where
    runPointerHandler handler event =
      fmap (`runWidgetAction` env) (handler event)
    runKeyHandler handler event =
      fmap (`runWidgetAction` env) (handler event)
