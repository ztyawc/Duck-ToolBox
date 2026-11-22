// SPDX-License-Identifier: GPL-3.0-only
package dev.duck.toolbox.timezone;

import android.util.Log;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.TimeZone;

/** Immutable process-local timezone policy; never changes device settings or epoch clock. */
public final class TimezoneHooks {
    public interface Hooker {
        Method hook(Method target, Object owner, Method callback);
    }

    public interface Deoptimizer {
        boolean deoptimize(Method method);
    }

    private final String id;
    private final TimeZone zone;
    private final ZoneId zoneId;
    private final android.icu.util.TimeZone icuZone;
    private final List<Hook> installed = new ArrayList<>();

    private TimezoneHooks(
        String id,
        TimeZone zone,
        ZoneId zoneId,
        android.icu.util.TimeZone icuZone
    ) {
        this.id = id;
        this.zone = zone;
        this.zoneId = zoneId;
        this.icuZone = icuZone;
    }

    public static TimezoneHooks create(String id) {
        if (id == null || id.isEmpty()) return null;
        try {
            ZoneId zoneId = ZoneId.of(id);
            TimeZone zone = TimeZone.getTimeZone(id);
            if (!"UTC".equals(id) && !"GMT".equals(id) && "GMT".equals(zone.getID())) {
                Log.w("DuckTimezone", "Unavailable Java timezone data: " + id);
                return null;
            }
            android.icu.util.TimeZone icu = null;
            try {
                android.icu.util.TimeZone candidate = android.icu.util.TimeZone.getTimeZone(id);
                if (candidate != null
                    && ("UTC".equals(id) || "GMT".equals(id)
                        || (!"GMT".equals(candidate.getID())
                            && !"Etc/Unknown".equals(candidate.getID())))) {
                    icu = (android.icu.util.TimeZone) candidate.clone();
                }
            } catch (LinkageError | RuntimeException ignored) {
                Log.w("DuckTimezone", "Optional ICU timezone data unavailable");
            }
            return new TimezoneHooks(id, (TimeZone) zone.clone(), zoneId, icu);
        } catch (RuntimeException error) {
            Log.w("DuckTimezone", "Rejected unsupported timezone: " + id);
            return null;
        }
    }

    public int installHooks(Hooker hooker, Deoptimizer deoptimizer) {
        TimeZone.setDefault((TimeZone) zone.clone());
        if (icuZone != null) {
            preselectIcuDefault();
        }
        System.setProperty("user.timezone", id);
        methods(hooker, TimeZone.class, "getDefault", Kind.ZONE);
        optional(hooker, "java.util.TimeZone", "getDefaultRef", Kind.ZONE_REF);
        methods(hooker, TimeZone.class, "setDefault", Kind.SET_ZONE);
        methods(hooker, ZoneId.class, "systemDefault", Kind.ZONE_ID);
        if (icuZone != null) {
            optional(hooker, "android.icu.util.TimeZone", "getDefault", Kind.ICU_ZONE);
            optional(hooker, "android.icu.util.TimeZone", "setDefault", Kind.SET_ICU_ZONE);
        }
        optional(hooker, "android.text.format.Time", "getCurrentTimezone", Kind.ZONE_STRING);
        deoptimize(deoptimizer, "android.app.ActivityThread",
            "updateTimeZone", "handleConfigurationChanged");
        return installed.size();
    }

    public String property(String name, boolean java) {
        if (name == null) return null;
        if (java) return "user.timezone".equals(name) ? id : null;
        return "persist.sys.timezone".equals(name) ? id : null;
    }

    private void preselectIcuDefault() {
        for (String name : new String[]{"setDefault", "setICUDefault"}) {
            try {
                Method setter = android.icu.util.TimeZone.class.getDeclaredMethod(
                    name, android.icu.util.TimeZone.class);
                setter.setAccessible(true);
                setter.invoke(null, icuZone.clone());
                return;
            } catch (ReflectiveOperationException | LinkageError | RuntimeException ignored) {
            }
        }
        Log.w("DuckTimezone", "Optional ICU default timezone could not be preselected");
    }

    private void optional(Hooker hooker, String clazz, String name, Kind kind) {
        try { methods(hooker, Class.forName(clazz), name, kind); }
        catch (ClassNotFoundException | LinkageError | RuntimeException ignored) {
            Log.w("DuckTimezone", "Optional timezone interface unavailable: " + clazz);
        }
    }

    private void deoptimize(Deoptimizer deoptimizer, String clazz, String... names) {
        try {
            for (Method method : Class.forName(clazz).getDeclaredMethods()) {
                if (Arrays.asList(names).contains(method.getName())) {
                    method.setAccessible(true);
                    if (!deoptimizer.deoptimize(method)) {
                        Log.w("DuckTimezone", "Framework caller could not be deoptimized");
                    }
                }
            }
        } catch (ClassNotFoundException | LinkageError | RuntimeException ignored) {
            Log.w("DuckTimezone", "Framework caller unavailable: " + clazz);
        }
    }

    private void methods(Hooker hooker, Class<?> clazz, String name, Kind kind) {
        for (Method method : clazz.getDeclaredMethods()) {
            if (!method.getName().equals(name) || Modifier.isAbstract(method.getModifiers())) continue;
            if ((kind == Kind.ZONE || kind == Kind.ZONE_REF || kind == Kind.ZONE_ID
                || kind == Kind.ICU_ZONE || kind == Kind.ZONE_STRING)
                && method.getParameterCount() != 0) continue;
            if (kind == Kind.ZONE_STRING && method.getReturnType() != String.class) continue;
            if (kind == Kind.SET_ZONE
                && (method.getParameterCount() != 1 || method.getParameterTypes()[0] != TimeZone.class)) {
                continue;
            }
            if (kind == Kind.SET_ICU_ZONE
                && (method.getParameterCount() != 1
                    || method.getParameterTypes()[0] != android.icu.util.TimeZone.class)) {
                continue;
            }
            Hook callback = new Hook(this, kind, Modifier.isStatic(method.getModifiers()));
            try {
                method.setAccessible(true);
                Method receiver = Hook.class.getDeclaredMethod("invoke", Object[].class);
                synchronized (callback) {
                    Method backup = hooker.hook(method, callback, receiver);
                    if (backup != null) {
                        backup.setAccessible(true);
                        callback.original = backup;
                        installed.add(callback);
                    } else {
                        Log.w("DuckTimezone", "Unavailable timezone hook: " + clazz.getName() + "." + name);
                    }
                }
            } catch (ReflectiveOperationException | RuntimeException error) {
                Log.w("DuckTimezone", "Timezone hook rejected: " + clazz.getName() + "." + name);
            }
        }
    }

    public enum Kind {
        ZONE,
        ZONE_REF,
        SET_ZONE,
        ZONE_ID,
        ICU_ZONE,
        SET_ICU_ZONE,
        ZONE_STRING
    }

    public static final class Hook {
        private final TimezoneHooks policy;
        private final Kind kind;
        private final boolean isStatic;
        private Method original;

        public Hook(TimezoneHooks policy, Kind kind, boolean isStatic) {
            this.policy = policy;
            this.kind = kind;
            this.isStatic = isStatic;
        }

        public Object invoke(Object[] args) throws Throwable {
            switch (kind) {
                case ZONE:
                    return policy.zone.clone();
                case ZONE_REF:
                    return (TimeZone) policy.zone.clone();
                case ZONE_ID:
                    return policy.zoneId;
                case ICU_ZONE:
                    return policy.icuZone.clone();
                case ZONE_STRING:
                    return policy.id;
                default:
                    break;
            }
            int offset = isStatic ? 0 : 1;
            Object receiver = isStatic ? null : args[0];
            Object[] parameters = Arrays.copyOfRange(args, offset, args.length);
            if (kind == Kind.SET_ZONE && parameters.length == 1) {
                parameters[0] = policy.zone.clone();
            } else if (kind == Kind.SET_ICU_ZONE && parameters.length == 1) {
                parameters[0] = policy.icuZone.clone();
            }
            Method backup;
            synchronized (this) { backup = original; }
            if (backup == null) throw new IllegalStateException("Timezone hook backup unavailable");
            try { return backup.invoke(receiver, parameters); }
            catch (InvocationTargetException failure) { throw failure.getCause(); }
        }
    }
}
