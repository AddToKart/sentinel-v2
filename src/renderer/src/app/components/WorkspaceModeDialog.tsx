import { Cloud, FolderRoot, X } from 'lucide-react'

import type { WorkspaceMode } from '@shared/types'

interface WorkspaceModeDialogProps {
  candidatePath: string | null
  open: boolean
  onClose: () => void
  onConfirm: (mode: WorkspaceMode) => void
}

export function WorkspaceModeDialog({
  candidatePath,
  open,
  onClose,
  onConfirm
}: WorkspaceModeDialogProps): JSX.Element | null {
  if (!open || !candidatePath) {
    return null
  }

  return (
    <div className="fixed inset-0 z-[160] flex items-center justify-center bg-black/60 px-4 backdrop-blur-sm">
      <div className="w-full max-w-2xl border border-white/10 bg-[#081019] shadow-[0_24px_80px_rgba(0,0,0,0.55)]">
        <div className="flex items-start justify-between gap-4 border-b border-white/10 px-5 py-4">
          <div>
            <div className="text-[10px] font-semibold uppercase tracking-[0.24em] text-sentinel-mist">
              Workspace Mode
            </div>
            <h2 className="mt-1 text-lg font-semibold text-white">
              Choose how this project should run
            </h2>
            <p className="mt-2 text-sm text-sentinel-mist">
              {candidatePath}
            </p>
          </div>

          <button
            className="inline-flex h-8 w-8 items-center justify-center border border-white/10 text-sentinel-mist transition hover:border-white/20 hover:text-white"
            onClick={onClose}
            type="button"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="grid gap-3 p-5 md:grid-cols-2">
          <button
            className="group border border-sentinel-accent/25 bg-sentinel-accent/8 p-5 text-left transition hover:border-sentinel-accent/45 hover:bg-sentinel-accent/12"
            onClick={() => onConfirm('local')}
            type="button"
          >
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center border border-sentinel-accent/25 bg-black/25 text-sentinel-accent">
                <FolderRoot className="h-5 w-5" />
              </div>
              <div>
                <div className="text-sm font-semibold text-white">Local Workspace</div>
                <div className="text-[11px] uppercase tracking-[0.18em] text-sentinel-accent/90">
                  Embedded Tauri backend
                </div>
              </div>
            </div>
            <p className="mt-4 text-sm leading-6 text-sentinel-mist">
              Run sessions and terminals directly on this machine with the current local Sentinel runtime.
            </p>
          </button>

          <button
            className="group border border-sky-400/20 bg-sky-400/8 p-5 text-left transition hover:border-sky-400/40 hover:bg-sky-400/12"
            onClick={() => onConfirm('cloud')}
            type="button"
          >
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center border border-sky-400/25 bg-black/25 text-sky-300">
                <Cloud className="h-5 w-5" />
              </div>
              <div>
                <div className="text-sm font-semibold text-white">Cloud Workspace</div>
                <div className="text-[11px] uppercase tracking-[0.18em] text-sky-200/90">
                  Remote session routing
                </div>
              </div>
            </div>
            <p className="mt-4 text-sm leading-6 text-sentinel-mist">
              Route this workspace through Sentinel Cloud. This requires the standalone cloud backend to be running and reachable.
            </p>
          </button>
        </div>
      </div>
    </div>
  )
}
