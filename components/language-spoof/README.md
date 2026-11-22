# Process-local language and timezone spoofing

The Miscellaneous tools configure one language and/or one IANA timezone per
`(Android user ID, package name)`. Language rules live in
`/data/adb/duck-toolbox/var/misc/language/rules.json` and timezone rules live in
`/data/adb/duck-toolbox/var/misc/timezone/rules.json`; each tool has an independent
activation record controlling whether new processes receive its rules. Saving does not enable
injection. Disabling preserves the rules, works with malformed rule JSON, and changes
the revision even if already disabled so stale enable requests cannot undo a stop.
Already running processes must be fully closed and reopened after any change.

The CLI exposes `duckd misc language status | save --stdin-json | enable --stdin-json | disable`
and `duckd misc timezone status | save --stdin-json | enable --stdin-json | disable`.
Language save input is
`{"revision":"<status revision>","config":{"rules":[{"package_name":"com.example.app","user_id":0,"locale":"en-US"}]}}`;
timezone save input uses `"timezone":"America/New_York"` (canonical regional IANA IDs or `UTC`).
Enable input is `{"revision":"<latest status revision>"}`. The revision includes
both rules and activation state; old requests fail instead of undoing a later stop.

The independent native/Java component joins the existing Zygisk v4 dispatcher using
channel ID 2. It does not require or enable the Device Faker feature. Language and
timezone spoofing share this single Zygisk channel, LSPlant initialization and
ShadowHook property map so both can be enabled on the same app without duplicate-hook
conflicts. It runs alongside
PI, and the dispatcher keeps the shared library loaded while any engine owns callbacks.
No hooks run in system_server. System, isolated and SDK-sandbox UIDs are excluded.
The app data directory, process package and numeric UID user scope must agree before
the companion selects a rule. Private companion reads are size bounded and use a
two-second exchange deadline. Missing, corrupt or disabled rules skip injection.

## Language and timezone surfaces

- Java `Locale` and `LocaleList` default getters and setters, including locale categories.
- Resource configuration reads and updates. Copies preserve unrelated configuration
  fields; the locale and layout direction follow the selected rule.
- Android `LocaleManager` system and own-application locale reads when present, and the
  framework configuration read through the ActivityManager Java proxy.
- Java `TimeZone` (`getDefault`, `getDefaultRef`, `setDefault`), `java.time.ZoneId.systemDefault()`,
  `android.icu.util.TimeZone` (`getDefault`, `setDefault`) and `android.text.format.Time.getCurrentTimezone()`.
  Mutable `TimeZone` getters return defensive clones so application mutations do not
  contaminate subsequent reads.
- Java `System` language/timezone properties (`user.language`, `user.country`, `user.variant`,
  `user.script`, `user.timezone`) and Android `SystemProperties` locale/timezone properties
  (`persist.sys.locale`, `ro.product.locale*`, `persist.sys.language`, `persist.sys.country`,
  `persist.sys.timezone`).
- Native `TZ` environment variable + `tzset()` (covering bionic `localtime` / `localtime_r` /
  `mktime` / `strftime`) and common bionic property getters, reads and read callbacks.
  A pre-existing unique native property hook is preserved when another hook cannot be installed.

LSPlant handles ART method replacement. ShadowHook supplies native trampolines and
the existing xDL implementation resolves ART symbols; a small prefix-search adapter
supports LSPlant's symbol contract. Known framework configuration callers are
deoptimized to reduce bypasses from inlining. Arbitrary precompiled callers, cached
values, direct Binder transactions, native ICU calls and custom app language/timezone logic
can still bypass these surfaces. This is not an anti-detection guarantee. No device
language, device timezone, system clock, system property, application data or Android
per-app setting is written. Regional content and layout may change; advertisement removal
is not guaranteed.

## Dependencies and validation

The stable LSPlant 6.4 documentation covers only through Android 15 DP1. The pinned
upstream revision used here documents Android 5–17 and requires NDK r30, libcxx modules
and CMake 3.28 or newer; the combined build uses the installed SDK CMake 4.1.2. LSPlant,
DexBuilder and parallel-hashmap revisions are explicit in the build script. Their
LGPL-3.0 and Apache-2.0 licenses accompany distributions; the build script retrieves
the pinned sources, and packaged source links identify the exact build commit.

Run `python3 scripts/build-play-integrity.py` for the ARM32/ARM64 release build,
`python3 scripts/test-language-spoof.py` for native selection/transport and host-JVM
callback checks, and `python3 scripts/test-zygisk-runtime.py` for dispatcher lifecycle
and descriptor ownership. The host tests do not prove ART or OEM compatibility.

On a test device with working Zygisk, select an ordinary test application and compare
`Locale.getDefault`, both locale categories, `LocaleList.getDefault`, app/system
resource locales, LocaleManager (API 33+) and native locale property reads. Recreate
the Activity, rotate the device, change the device language and restart a remote
process to check configuration updates. Repeat in a secondary Android user, then
disable and restart to verify restoration. An unselected app and system_server must
continue to see the original language. Inspect the `DuckLanguage` log for rejected
hooks. Device validation is required before claiming support for a specific ROM.

The Android framework language API is documented at
<https://developer.android.com/guide/topics/resources/app-languages>.
LSPlant's hook, deoptimization and symbol-resolver contracts are documented at
<https://github.com/LSPosed/LSPlant>.
