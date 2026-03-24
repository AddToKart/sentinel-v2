export function BridgeUnavailableScreen(): JSX.Element {
  return (
    <div className="flex h-[100dvh] w-screen items-center justify-center overflow-hidden bg-[#060a0f] px-6 text-white">
      <div className="max-w-xl border border-white/10 bg-black/30 p-6">
        <div className="text-xs font-semibold uppercase tracking-[0.28em] text-sentinel-mist">Sentinel</div>
        <h1 className="mt-3 text-xl font-semibold text-white">Desktop Bridge Unavailable</h1>
        <p className="mt-3 text-sm leading-6 text-sentinel-mist">
          `window.sentinel` is only initialized when the Sentinel UI is running inside Tauri. If you open the renderer in a normal browser tab, the desktop bridge does not exist.
        </p>
        <div className="mt-4 border border-white/10 bg-black/30 px-3 py-3 text-xs text-sentinel-mist">
          Start Sentinel through Tauri, or add a mocked web bridge for browser-only development.
        </div>
      </div>
    </div>
  )
}
