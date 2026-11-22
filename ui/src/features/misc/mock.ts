import { MockFailure, type MockModule } from "@/core/mock"
import type { LanguageRule, LanguageStatus } from "./language/api"
import type { TimezoneRule, TimezoneStatus } from "./timezone/api"

const params = new URLSearchParams(location.search)
const languageScenario = params.get("language")
const timezoneScenario = params.get("timezone")

let languageSequence = 0
let languageEnabled = false
let languageRules: LanguageRule[] =
  languageScenario === "rules"
    ? [{ package_name: "com.tencent.mm", user_id: 0, locale: "en-US" }]
    : []
const languageRevision = () => `language-${languageSequence}`
const languageStatus = (): LanguageStatus => ({
  config: languageScenario === "corrupt" ? null : { rules: structuredClone(languageRules) },
  config_error: languageScenario === "corrupt" ? "invalid_config" : null,
  revision: languageRevision(),
  enabled: languageEnabled,
  payload_installed: languageScenario !== "unavailable",
  parent_active: true,
  restart_required: true,
})

let timezoneSequence = 0
let timezoneEnabled = false
let timezoneRules: TimezoneRule[] =
  timezoneScenario === "rules"
    ? [{ package_name: "com.tencent.mm", user_id: 0, timezone: "America/New_York" }]
    : []
const timezoneRevision = () => `timezone-${timezoneSequence}`
const timezoneStatus = (): TimezoneStatus => ({
  config: timezoneScenario === "corrupt" ? null : { rules: structuredClone(timezoneRules) },
  config_error: timezoneScenario === "corrupt" ? "invalid_config" : null,
  revision: timezoneRevision(),
  enabled: timezoneEnabled,
  payload_installed: timezoneScenario !== "unavailable",
  parent_active: true,
  restart_required: true,
})

function check<T>(input: unknown, expected: string): T {
  const value = input as T & { revision: string }
  if (value.revision !== expected) throw new MockFailure("stale_revision", "Refresh before saving")
  return value
}

const mock: MockModule = {
  feature: { id: "misc", contract: 2 },
  handlers: {
    "misc.language.status": languageStatus,
    "misc.language.save": (_args, _flags, input) => {
      const value = check<{ config: { rules: LanguageRule[] } }>(input, languageRevision())
      languageRules = structuredClone(value.config.rules)
      languageSequence += 1
      return languageStatus()
    },
    "misc.language.enable": (_args, _flags, input) => {
      check(input, languageRevision())
      if (languageScenario === "corrupt")
        throw new MockFailure("invalid_config", "Invalid configuration")
      if (!languageRules.length) throw new MockFailure("empty_rules", "Add an application first")
      languageEnabled = true
      languageSequence += 1
      return languageStatus()
    },
    "misc.language.disable": () => {
      languageEnabled = false
      languageSequence += 1
      return { enabled: false, restart_required: true }
    },
    "misc.timezone.status": timezoneStatus,
    "misc.timezone.save": (_args, _flags, input) => {
      const value = check<{ config: { rules: TimezoneRule[] } }>(input, timezoneRevision())
      timezoneRules = structuredClone(value.config.rules)
      timezoneSequence += 1
      return timezoneStatus()
    },
    "misc.timezone.enable": (_args, _flags, input) => {
      check(input, timezoneRevision())
      if (timezoneScenario === "corrupt")
        throw new MockFailure("invalid_config", "Invalid configuration")
      if (!timezoneRules.length) throw new MockFailure("empty_rules", "Add an application first")
      timezoneEnabled = true
      timezoneSequence += 1
      return timezoneStatus()
    },
    "misc.timezone.disable": () => {
      timezoneEnabled = false
      timezoneSequence += 1
      return { enabled: false, restart_required: true }
    },
  },
}
export default mock
