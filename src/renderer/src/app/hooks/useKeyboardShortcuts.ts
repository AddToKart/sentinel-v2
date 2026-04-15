import { useEffect } from 'react'

interface UseKeyboardShortcutsOptions {
  onToggleConsole: () => void
  onToggleGlobalActionBar: () => void
  onToggleIdeTerminal: () => void
  onToggleChangesManager?: () => void
}

export function useKeyboardShortcuts({
  onToggleConsole,
  onToggleGlobalActionBar,
  onToggleIdeTerminal,
  onToggleChangesManager
}: UseKeyboardShortcutsOptions): void {
  useEffect(() => {
    function onKey(event: KeyboardEvent): void {
      if (event.ctrlKey && event.code === 'KeyK') {
        event.preventDefault()
        onToggleGlobalActionBar()
        return
      }

      if (event.ctrlKey && !event.altKey && !event.shiftKey) {
        if (event.code === 'Backquote') {
          event.preventDefault()
          onToggleIdeTerminal()
        }

        if (event.code === 'KeyJ') {
          event.preventDefault()
          onToggleConsole()
        }

        if (event.code === 'KeyM' && onToggleChangesManager) {
          event.preventDefault()
          onToggleChangesManager()
        }
      }
    }

    window.addEventListener('keydown', onKey, { capture: true })
    return () => window.removeEventListener('keydown', onKey, { capture: true })
  }, [onToggleConsole, onToggleGlobalActionBar, onToggleIdeTerminal, onToggleChangesManager])
}
