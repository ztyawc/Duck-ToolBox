import { useQuery, useQueryClient } from "@tanstack/vue-query"
import { computed, ref, watch } from "vue"
import { onBeforeRouteLeave } from "vue-router"
import { useI18n } from "vue-i18n"
import { notifyError, notifySuccess } from "@/core/notify"
import { useMiscI18n } from "../i18n"
import { useLanguageConfirmation } from "../language/confirmation"
import { timezoneApi, type TimezoneRule, type TimezoneStatus } from "./api"

export function useTimezoneEditor() {
  const { t } = useMiscI18n()
  const { t: coreT } = useI18n()
  const confirmation = useLanguageConfirmation()
  const client = useQueryClient()
  const key = ["misc", "timezone"]
  const status = useQuery({ queryKey: key, queryFn: timezoneApi.status })
  const draft = ref<TimezoneRule[]>([])
  const base = ref("[]")
  const revision = ref("")
  const busy = ref(false)
  const stopping = ref(false)
  const dirty = computed(() => JSON.stringify(draft.value) !== base.value)
  const changed = computed(
    () => !!status.data.value && status.data.value.revision !== revision.value,
  )
  function load(data: TimezoneStatus) {
    draft.value = (data.config?.rules ?? []).map((rule) => ({ ...rule }))
    base.value = JSON.stringify(draft.value)
    revision.value = data.revision
  }
  watch(
    status.data,
    (data) => {
      if (data && !dirty.value) load(data)
    },
    { immediate: true },
  )
  async function discard() {
    return (
      !dirty.value ||
      confirmation.confirm({ title: t("timezone.discard"), description: t("timezone.unsaved") })
    )
  }
  onBeforeRouteLeave(async () => !busy.value && !stopping.value && (await discard()))
  async function refresh() {
    if (busy.value || stopping.value || !(await discard())) return
    busy.value = true
    try {
      const result = await status.refetch({ throwOnError: true })
      if (result.data) load(result.data)
    } catch (error) {
      notifyError(error)
    } finally {
      busy.value = false
    }
  }
  async function action(work: () => Promise<TimezoneStatus>) {
    if (busy.value || stopping.value) return
    busy.value = true
    try {
      await client.cancelQueries({ queryKey: key })
      const data = await work()
      load(data)
      client.setQueryData(key, data)
      notifySuccess(t("timezone.saved"), t("timezone.restart"))
    } catch (error) {
      notifyError(error)
    } finally {
      busy.value = false
    }
  }
  async function enable() {
    if (dirty.value || changed.value || !status.data.value) return
    const expected = revision.value
    if (
      await confirmation.confirm({
        title: t("timezone.enable"),
        description: t("timezone.limits"),
        confirmLabel: coreT("core.actions.confirm"),
      })
    ) {
      await action(() => timezoneApi.enable(expected))
    }
  }
  async function disable() {
    if (busy.value || stopping.value) return
    stopping.value = true
    try {
      await timezoneApi.disable()
      client.setQueryData<TimezoneStatus>(key, (old) => (old ? { ...old, enabled: false } : old))
      const result = await status.refetch({ throwOnError: true })
      if (result.data && JSON.stringify(result.data.config?.rules ?? []) === base.value) {
        revision.value = result.data.revision
      } else if (result.data && !dirty.value) load(result.data)
      notifySuccess(t("timezone.disabled"), t("timezone.restart"))
    } catch (error) {
      notifyError(error)
    } finally {
      stopping.value = false
    }
  }
  return {
    confirmation,
    status,
    draft,
    dirty,
    changed,
    busy,
    stopping,
    refresh,
    enable,
    disable,
    save: () =>
      action(() =>
        timezoneApi.save(
          revision.value,
          draft.value.map((rule) => ({ ...rule })),
        ),
      ),
  }
}
