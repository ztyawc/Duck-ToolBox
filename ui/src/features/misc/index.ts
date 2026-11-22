import { Shapes } from "@lucide/vue"
import { defineFeature } from "@/core/features"
import { localeFiles } from "@/core/i18n"
import { NAMESPACE } from "./i18n"

export default defineFeature({
  id: "misc",
  namespace: NAMESPACE,
  contract: 2,
  icon: Shapes,
  order: 90,
  routes: [
    { path: "", name: "misc", component: () => import("./pages/MiscPage.vue") },
    {
      path: "language",
      name: "misc.language",
      component: () => import("./language/LanguagePage.vue"),
      meta: { titleKey: `${NAMESPACE}.language.title`, backTo: "/misc" },
    },
    {
      path: "timezone",
      name: "misc.timezone",
      component: () => import("./timezone/TimezonePage.vue"),
      meta: { titleKey: `${NAMESPACE}.timezone.title`, backTo: "/misc" },
    },
  ],
  messages: localeFiles(import.meta.glob("./locales/*.json")),
})
