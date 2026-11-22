// @vitest-environment node
import { afterEach, describe, expect, it } from "vitest"
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import { bridge, setBridge } from "@/core/bridge"
import { CONTRACT, hasDuckd, installHost, type Host } from "@/core/testing/host"
import { timezoneApi } from "./api"

let host: Host | undefined
afterEach(() => {
  setBridge(null)
  host?.dispose()
  host = undefined
})

describe.skipIf(!hasDuckd)("misc timezone API contract", CONTRACT, () => {
  it("normalizes canonical IANA IDs, enables explicitly and rejects stale activation after stop", async () => {
    host = installHost()
    const module = bridge().hostModule()!.dir
    mkdirSync(path.join(module, "play-integrity"), { recursive: true })
    writeFileSync(path.join(module, "play-integrity/timezone-spoof.version"), "1\n")
    writeFileSync(path.join(module, "play-integrity/classes.dex"), "dex fixture")
    mkdirSync(path.join(module, "zygisk"), { recursive: true })
    writeFileSync(path.join(module, "zygisk/arm64-v8a.so"), "ELF fixture")

    const initial = await timezoneApi.status()
    expect(initial.enabled).toBe(false)
    const saved = await timezoneApi.save(initial.revision, [
      { package_name: "com.example.app", user_id: 0, timezone: "america/new_york" },
      { package_name: "com.example.app", user_id: 10, timezone: "asia/tokyo" },
    ])
    expect(saved.config?.rules.map((rule) => rule.timezone)).toEqual([
      "America/New_York",
      "Asia/Tokyo",
    ])
    expect(saved.enabled).toBe(false)

    await timezoneApi.disable()
    await expect(timezoneApi.enable(saved.revision)).rejects.toMatchObject({
      code: "stale_revision",
    })
    const stopped = await timezoneApi.status()
    expect((await timezoneApi.enable(stopped.revision)).enabled).toBe(true)
    await timezoneApi.disable()
    expect((await timezoneApi.status()).config?.rules).toHaveLength(2)
  })

  it("rejects unknown or POSIX timezone strings and allows stopping corrupt rules", async () => {
    host = installHost({})
    const current = await timezoneApi.status()
    await expect(
      timezoneApi.save(current.revision, [
        { package_name: "com.example.app", user_id: 0, timezone: "Mars/Olympus" },
      ]),
    ).rejects.toMatchObject({ code: "invalid_config" })

    const saved = await timezoneApi.save(current.revision, [
      { package_name: "com.example.app", user_id: 0, timezone: "America/New_York" },
    ])
    writeFileSync(path.join(bridge().hostModule()!.dir, "var/misc/timezone/rules.json"), "corrupt")
    expect((await timezoneApi.status()).config_error).toBe("invalid_config")
    expect((await timezoneApi.disable()).enabled).toBe(false)
    await expect(timezoneApi.save(saved.revision, [])).rejects.toMatchObject({
      code: "stale_revision",
    })
  })
})
