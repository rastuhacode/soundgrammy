import { AudioCacheSection } from './sections/AudioCacheSection'
import { DiagnosticsSection } from './sections/DiagnosticsSection'
import { FullscreenPlayerSection } from './sections/FullscreenPlayerSection'
import { IntegrationsSection } from './sections/IntegrationsSection'
import { ListeningStatisticsSection } from './sections/ListeningStatisticsSection'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

export interface SettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-lg">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-4 pr-12">
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div className="min-h-0 grow overflow-x-hidden overflow-y-auto px-4 py-4">
          <div className="flex min-w-0 flex-col gap-2">
            <AudioCacheSection />
            <ListeningStatisticsSection />
            <IntegrationsSection />
            <DiagnosticsSection />
            <FullscreenPlayerSection />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
