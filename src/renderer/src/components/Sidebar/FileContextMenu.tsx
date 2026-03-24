import { createPortal } from 'react-dom'

interface FileContextMenuProps {
  filePath: string
  x: number
  y: number
  onOpenInSystemEditor: () => void
  onRevealInExplorer: () => void
}

export function FileContextMenu({
  filePath,
  x,
  y,
  onOpenInSystemEditor,
  onRevealInExplorer
}: FileContextMenuProps): JSX.Element | null {
  if (typeof document === 'undefined') {
    return null
  }

  return createPortal(
    <div
      className="fixed z-50 min-w-[220px] border border-white/10 bg-[#0b1219] p-1.5 shadow-terminal backdrop-blur-2xl"
      style={{ left: x, top: y }}
    >
      <button
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm text-white transition hover:bg-white/[0.06]"
        onClick={onRevealInExplorer}
        type="button"
      >
        <span>Reveal in File Explorer</span>
        <span className="font-mono text-[11px] text-sentinel-mist">explorer</span>
      </button>
      <button
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm text-white transition hover:bg-white/[0.06]"
        onClick={onOpenInSystemEditor}
        type="button"
      >
        <span>Open in System Editor</span>
        <span className="font-mono text-[11px] text-sentinel-mist">system</span>
      </button>
      <div className="px-3 pt-1 text-[10px] text-sentinel-mist/70 truncate" title={filePath}>
        {filePath}
      </div>
    </div>,
    document.body
  )
}
