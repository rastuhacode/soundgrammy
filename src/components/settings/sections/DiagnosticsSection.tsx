import { SettingsSection } from '../SettingsSection'
import { DiagnosticsForm } from '../forms/DiagnosticsForm'

export function DiagnosticsSection() {
  return (
    <SettingsSection title="Diagnostics">
      <DiagnosticsForm />
    </SettingsSection>
  )
}
