import { SettingsSection } from '../SettingsSection'
import { AudioCacheForm } from '../forms/AudioCacheForm'

export function AudioCacheSection() {
  return (
    <SettingsSection title="Audio cache">
      <AudioCacheForm />
    </SettingsSection>
  )
}
