import { SettingsSection } from '../SettingsSection'
import { FullscreenPlayerForm } from '../forms/FullscreenPlayerForm'

export function FullscreenPlayerSection() {
  return (
    <SettingsSection title="Fullscreen player">
      <FullscreenPlayerForm />
    </SettingsSection>
  )
}
