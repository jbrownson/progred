import * as React from "react"

// TODO I wish there was a better way to do this than having to manually override these, too coupled
export function stopPropagationForTextInputs(e: React.KeyboardEvent<HTMLInputElement> | React.KeyboardEvent<HTMLTextAreaElement>) {
  const selectionStart = e.currentTarget.selectionStart ?? 0
  const selectionEnd = e.currentTarget.selectionEnd ?? 0
  switch (e.key) {
      case "ArrowUp":
      case "ArrowLeft":
        if (selectionStart > 0 || selectionEnd > 0)
          e.stopPropagation()
        break
      case "ArrowDown":
      case "ArrowRight":
        if (selectionStart < e.currentTarget.value.length || selectionEnd < e.currentTarget.value.length)
          e.stopPropagation()
        break
      case "Delete":
      case "Backspace":
        e.stopPropagation()
        break }}
