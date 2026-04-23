{-# LANGUAGE OverloadedStrings #-}

module Main (main) where

import Control.Exception (bracket, bracket_)
import Control.Monad (unless, when)
import Control.Monad.IO.Class (liftIO)
import Control.Monad.Managed (managed, managed_, runManaged)
import DearImGui qualified as ImGui
import DearImGui.FontAtlas qualified as ImGui.Font
import DearImGui.OpenGL3 qualified as ImGui.GL
import DearImGui.SDL qualified as ImGui.SDL
import DearImGui.SDL.OpenGL qualified as ImGui.SDL.GL
import Graphics.GL qualified as GL
import SDL qualified

main :: IO ()
main = do
  -- Init only what we need. SDL.initializeAll pulls in the joystick
  -- subsystem, which opens IOKit's HID manager and triggers macOS's
  -- Input Monitoring permission prompt. Video + events alone is silent.
  SDL.initialize [SDL.InitVideo, SDL.InitEvents]
  runManaged $ do
    -- macOS only supports modern GLSL with an explicit 3.2+ Core
    -- profile. SDL's default gives the legacy 2.1 context.
    let glConfig = SDL.defaultOpenGL
          { SDL.glProfile = SDL.Core SDL.Normal 3 3
          }
        cfg = SDL.defaultWindow
          { SDL.windowGraphicsContext = SDL.OpenGLContext glConfig
          , SDL.windowInitialSize = SDL.V2 900 600
          , SDL.windowResizable = True
          , SDL.windowHighDPI = True
          }
    window <- managed (bracket (SDL.createWindow "prototype-haskell-imgui" cfg) SDL.destroyWindow)
    glContext <- managed (bracket (SDL.glCreateContext window) SDL.glDeleteContext)
    _ <- managed (bracket ImGui.createContext ImGui.destroyContext)
    managed_ (bracket_ (ImGui.SDL.GL.sdl2InitForOpenGL window glContext) ImGui.SDL.sdl2Shutdown)
    managed_ (bracket_ ImGui.GL.openGL3Init ImGui.GL.openGL3Shutdown)
    -- Load a real font at 2x size so it stays crisp on Retina.
    _ <- liftIO (ImGui.Font.addFontFromFileTTF "/System/Library/Fonts/SFNS.ttf" 28 Nothing Nothing)
    liftIO (loop window)

loop :: SDL.Window -> IO ()
loop window = do
  shouldQuit <- pollEvents
  ImGui.GL.openGL3NewFrame
  ImGui.SDL.sdl2NewFrame
  ImGui.newFrame
  drawUI
  GL.glClearColor 0.10 0.10 0.12 1.0
  GL.glClear GL.GL_COLOR_BUFFER_BIT
  ImGui.render
  ImGui.GL.openGL3RenderDrawData =<< ImGui.getDrawData
  SDL.glSwapWindow window
  unless shouldQuit (loop window)

drawUI :: IO ()
drawUI = do
  _ <- ImGui.begin "Hello"
  ImGui.text "ImGui from Haskell."
  clicked <- ImGui.button "Click me"
  when clicked (putStrLn "clicked")
  ImGui.end

pollEvents :: IO Bool
pollEvents = drain False
  where
    drain quit = do
      mev <- ImGui.SDL.pollEventWithImGui
      case mev of
        Nothing -> pure quit
        Just ev -> case SDL.eventPayload ev of
          SDL.QuitEvent -> drain True
          _             -> drain quit
