import { AppHeader } from './app/components/AppHeader'
import { AppWorkspacePanels } from './app/components/AppWorkspacePanels'
import { BridgeUnavailableScreen } from './app/components/BridgeUnavailableScreen'
import { ErrorToast } from './app/components/ErrorToast'
import { WorkspaceModeDialog } from './app/components/WorkspaceModeDialog'
import { useAppController } from './app/hooks/useAppController'
import { ChangesManagerPanel } from './components/changes-manager/ChangesManagerPanel'
import { ConsoleDrawer } from './components/ConsoleDrawer'
import { GlobalActionBar } from './components/GlobalActionBar'

export default function App(): JSX.Element {
  const controller = useAppController()

  if (!controller.bridgeAvailable) {
    return <BridgeUnavailableScreen />
  }

  return (
    <div className="flex h-[100dvh] w-screen flex-col overflow-hidden bg-[#060a0f] text-white select-none">
      <ErrorToast
        message={controller.errorMessage}
        onDismiss={controller.clearErrorMessage}
      />

      <AppHeader {...controller.headerProps} />
      <AppWorkspacePanels {...controller.workspacePanelsProps} />

      <div
        className={`fixed inset-x-0 bottom-0 z-40 flex h-[36vh] flex-col overflow-hidden bg-[#060c14]/98 shadow-2xl backdrop-blur-2xl transition-transform duration-300 ease-in-out ${
          controller.consoleOpen ? 'translate-y-0 border-t border-sentinel-accent/20' : 'translate-y-full'
        }`}
      >
        <ConsoleDrawer
          entries={controller.activityLog}
          onToggleOpen={controller.toggleConsole}
          open={controller.consoleOpen}
        />
      </div>

      <GlobalActionBar
        actions={controller.globalActions}
        isOpen={controller.globalActionBarOpen}
        onClose={controller.closeGlobalActionBar}
      />
      <WorkspaceModeDialog {...controller.workspaceModeDialogProps} />
      <ChangesManagerPanel {...controller.changesManagerProps} />
    </div>
  )
}

