import { QueryClient, VueQueryPlugin } from "@tanstack/vue-query"
import { flushPromises, mount } from "@vue/test-utils"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { defineComponent, h } from "vue"
import { notifyError } from "@/core/notify"
import { timezoneApi, type TimezoneStatus } from "./api"
import { useTimezoneEditor } from "./useTimezoneEditor"

vi.mock("vue-router", () => ({ onBeforeRouteLeave: vi.fn() }))
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock("../i18n", () => ({ useMiscI18n: () => ({ t: (key: string) => key }) }))
vi.mock("@/core/notify", () => ({ notifyError: vi.fn(), notifySuccess: vi.fn() }))
vi.mock("./api", () => ({
  timezoneApi: { status: vi.fn(), save: vi.fn(), enable: vi.fn(), disable: vi.fn() },
}))

const key = ["misc", "timezone"]
const cleanups: (() => void)[] = []
const value = (
  revision: string,
  enabled = false,
  timezone = "America/New_York",
): TimezoneStatus => ({
  revision,
  enabled,
  config: { rules: [{ package_name: "com.example.app", user_id: 0, timezone }] },
  config_error: null,
  parent_active: true,
  payload_installed: true,
  restart_required: true,
})

beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(timezoneApi.status).mockResolvedValue(value("original"))
  vi.mocked(timezoneApi.disable).mockResolvedValue({ enabled: false, restart_required: true })
})
afterEach(() => cleanups.splice(0).forEach((cleanup) => cleanup()))

async function setup() {
  let editor!: ReturnType<typeof useTimezoneEditor>
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = mount(
    defineComponent({
      setup() {
        editor = useTimezoneEditor()
        return () => h("div")
      },
    }),
    { global: { plugins: [[VueQueryPlugin, { queryClient: client }]] } },
  )
  cleanups.push(() => {
    wrapper.unmount()
    client.clear()
  })
  await flushPromises()
  return { editor, client, wrapper }
}

it("uses the saved response as authoritative state without a second read", async () => {
  const { editor } = await setup()
  editor.draft.value[0]!.timezone = "Asia/Tokyo"
  vi.mocked(timezoneApi.save).mockResolvedValue(value("saved", false, "Asia/Tokyo"))
  await editor.save()
  await flushPromises()
  expect(editor.status.data.value?.revision).toBe("saved")
  expect(editor.changed.value).toBe(false)
  expect(editor.dirty.value).toBe(false)
  expect(timezoneApi.status).toHaveBeenCalledTimes(1)
})

it("retains the revision from before an enable confirmation opened", async () => {
  const { editor, client } = await setup()
  const enabling = editor.enable()
  await flushPromises()
  client.setQueryData(key, value("stopped-elsewhere"))
  await flushPromises()
  vi.mocked(timezoneApi.enable).mockRejectedValue(new Error("stale revision"))
  editor.confirmation.accept()
  await enabling
  expect(timezoneApi.enable).toHaveBeenCalledWith("original")
  expect(notifyError).toHaveBeenCalled()
})

it("can save retained local edits after an acknowledged stop", async () => {
  const { editor } = await setup()
  editor.draft.value[0]!.timezone = "Asia/Shanghai"
  vi.mocked(timezoneApi.status).mockResolvedValueOnce(value("stopped"))
  await editor.disable()
  await flushPromises()
  expect(editor.dirty.value).toBe(true)
  expect(editor.changed.value).toBe(false)
  vi.mocked(timezoneApi.save).mockResolvedValue(value("saved", false, "Asia/Shanghai"))
  await editor.save()
  expect(timezoneApi.save).toHaveBeenCalledWith("stopped", expect.any(Array))
})
