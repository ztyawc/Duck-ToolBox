// SPDX-License-Identifier: GPL-3.0-only
#include "language.hpp"
#include "transport.hpp"
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#ifndef DUCK_LANGUAGE_DATA
#define DUCK_LANGUAGE_DATA "/data/adb/duck-toolbox/var/misc/language"
#endif
#ifndef DUCK_TIMEZONE_DATA
#define DUCK_TIMEZONE_DATA "/data/adb/duck-toolbox/var/misc/timezone"
#endif
#ifndef DUCK_LANGUAGE_MODULE
#define DUCK_LANGUAGE_MODULE "/data/adb/modules/duck-toolbox"
#endif

namespace duck::language {
static bool authorized(int fd) {
    // Accept only the privileged peer supplied by the runtime. This checks the
    // descriptor's peer, not an end-user identity behind a runtime relay.
#ifdef __linux__
    ucred peer{};
    socklen_t size = sizeof(peer);
    return getsockopt(fd, SOL_SOCKET, SO_PEERCRED, &peer, &size) == 0
        && size == sizeof(peer) && peer.uid == geteuid();
#elif defined(__APPLE__)
    uid_t uid; gid_t gid;
    return getpeereid(fd, &uid, &gid) == 0 && uid == geteuid();
#else
    return false;
#endif
}
static bool absent(const char *path) {
    struct stat metadata{};
    return lstat(path, &metadata) != 0 && errno == ENOENT;
}
static std::vector<char> file(const char *path, size_t maximum) {
    int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
    if (fd < 0) return {};
    struct Close { int fd; ~Close() { close(fd); } } owner{fd};
    struct stat metadata{};
    if (fstat(fd, &metadata) || !S_ISREG(metadata.st_mode) || metadata.st_size <= 0 || static_cast<uint64_t>(metadata.st_size) > maximum) return {};
    std::vector<char> bytes(static_cast<size_t>(metadata.st_size));
    size_t offset = 0;
    while (offset < bytes.size()) {
        auto count = read(fd, bytes.data() + offset, bytes.size() - offset);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) return {};
        offset += static_cast<size_t>(count);
    }
    return bytes;
}
static std::string resolve(
    const char *enabled_path,
    const char *rules_path,
    const Target &target,
    std::string (*selector)(std::string_view, const Target &),
    std::vector<char> &snapshot
) {
    snapshot = file(enabled_path, 64);
    if (snapshot.empty() || !active(std::string_view(snapshot.data(), snapshot.size()))) return {};
    auto json = file(rules_path, 65536);
    if (json.empty()) return {};
    return selector(std::string_view(json.data(), json.size()), target);
}
bool exchange(int fd, const Target &target, Policy &policy, std::vector<char> &dex) {
    Transport transport(fd);
    uint32_t uid = target.uid;
    auto package = target.package;
    return transport.number(uid, true) && transport.blob(package, 255, true)
        && transport.blob(policy.locale, 12, false)
        && transport.blob(policy.timezone, 64, false)
        && !policy.empty()
        && transport.blob(dex, 4 * 1024 * 1024, false) && !dex.empty();
}
void companion(int fd) {
    if (!authorized(fd)) return;
    Transport transport(fd);
    Target target;
    uint32_t uid = 0;
    if (!transport.number(uid, false) || !transport.blob(target.package, 255, false)) return;
    target.uid = uid;
    Policy policy;
    std::vector<char> dex, lang_enabled, tz_enabled;
    if (absent(DUCK_LANGUAGE_MODULE "/disable")
        && absent(DUCK_LANGUAGE_MODULE "/remove")
        && uid % 100000 >= 10000 && uid % 100000 < 20000) {
        policy.locale = resolve(DUCK_LANGUAGE_DATA "/enabled", DUCK_LANGUAGE_DATA "/rules.json", target, select, lang_enabled);
        policy.timezone = resolve(DUCK_TIMEZONE_DATA "/enabled", DUCK_TIMEZONE_DATA "/rules.json", target, select_timezone, tz_enabled);
        if (!policy.empty()) dex = file(DUCK_LANGUAGE_MODULE "/play-integrity/classes.dex", 4 * 1024 * 1024);
        // A stop or parent disable during preparation invalidates the affected policy.
        if (!policy.locale.empty() && file(DUCK_LANGUAGE_DATA "/enabled", 64) != lang_enabled) policy.locale.clear();
        if (!policy.timezone.empty() && file(DUCK_TIMEZONE_DATA "/enabled", 64) != tz_enabled) policy.timezone.clear();
        if (dex.empty() || policy.empty()
            || !absent(DUCK_LANGUAGE_MODULE "/disable")
            || !absent(DUCK_LANGUAGE_MODULE "/remove")) {
            policy = {};
            dex.clear();
        }
    }
    if (transport.blob(policy.locale, 12, true)
        && transport.blob(policy.timezone, 64, true)
        && !policy.empty()) {
        transport.blob(dex, 4 * 1024 * 1024, true);
    }
}
}
