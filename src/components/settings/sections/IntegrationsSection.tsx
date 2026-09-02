import { SettingsSection } from '../SettingsSection'
import { LastFmSettingsForm } from '../forms/LastFmSettingsForm'
import { ProxySettingsForm } from '../forms/ProxySettingsForm'

export function IntegrationsSection() {
  return (
    <SettingsSection title="Integrations" contentClassName="flex flex-col gap-4">
      <LastFmSettingsForm />
      <div className="border-t border-border/60 pt-4">
        <ProxySettingsForm />
      </div>
    </SettingsSection>
  )
}
