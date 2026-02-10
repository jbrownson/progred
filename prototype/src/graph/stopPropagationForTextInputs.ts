import * as React from "react"

// TODO I wish there was a better way to do this than having to manually override these, too coupled
export function stopPropagationForTextInputs(e: React.KeyboardEvent<HTMLInputElement> | React.KeyboardEvent<HTMLTextAreaElement>) {
  switch (e.key) {
      case "ArrowUp":
      case "ArrowLeft":
        if (e.currentTarget.selectionStart > 0 || e.currentTarget.selectionEnd > 0)
          e.stopPropagation()
        break
      case "ArrowDown":
      case "ArrowRight":
        if (e.currentTarget.selectionStart < e.currentTarget.value.length || e.currentTarget.selectionEnd < e.currentTarget.value.length)
          e.stopPropagation()
        break
      case "Delete":
      case "Backspace":
        e.stopPropagation()
        break }}