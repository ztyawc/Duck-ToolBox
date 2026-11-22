<script setup lang="ts">
import { ref, watch } from "vue"
import { useI18n } from "vue-i18n"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog"
import { Field, FieldLabel, FieldDescription } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useMiscI18n } from "../i18n"
import AppPicker from "../language/AppPicker.vue"
import type { TimezoneRule } from "./api"

const props = defineProps<{ rule: TimezoneRule | null }>()
const open = defineModel<boolean>("open", { required: true })
const emit = defineEmits<{ save: [rule: TimezoneRule] }>()
const { t } = useMiscI18n()
const { t: coreT } = useI18n()
const packageName = ref("")
const userId = ref<string | number>(0)
const timezone = ref("America/New_York")
const picking = ref(false)
const presets = [
  "America/New_York",
  "America/Chicago",
  "America/Los_Angeles",
  "Europe/London",
  "Europe/Berlin",
  "Asia/Shanghai",
  "Asia/Tokyo",
  "Asia/Seoul",
  "Asia/Singapore",
  "UTC",
]
watch(open, (value) => {
  if (!value) return
  packageName.value = props.rule?.package_name ?? ""
  userId.value = props.rule?.user_id ?? 0
  timezone.value = props.rule?.timezone ?? "America/New_York"
  picking.value = false
})
function submit() {
  emit("save", {
    package_name: packageName.value.trim(),
    user_id: Number(userId.value),
    timezone: timezone.value.trim(),
  })
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent @open-auto-focus.prevent>
      <DialogHeader>
        <DialogTitle>{{ t("timezone.rule") }}</DialogTitle>
        <DialogDescription>{{ t("timezone.ruleHelp") }}</DialogDescription>
      </DialogHeader>
      <form class="flex min-w-0 flex-col gap-5" @submit.prevent="submit">
        <Field>
          <FieldLabel for="timezone-package">{{ t("timezone.package") }}</FieldLabel>
          <Input
            id="timezone-package"
            v-model="packageName"
            required
            maxlength="255"
            pattern="[a-zA-Z][a-zA-Z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+"
            dir="ltr"
            autocapitalize="off"
            spellcheck="false"
            placeholder="com.example.app"
          />
          <Button
            type="button"
            variant="outline"
            class="self-start"
            :aria-expanded="picking"
            @click="picking = !picking"
            >{{ t("timezone.pickApp") }}</Button
          >
          <AppPicker
            v-if="picking"
            @select="
              (name) => {
                packageName = name
                picking = false
              }
            "
          />
        </Field>
        <Field>
          <FieldLabel for="timezone-user">{{ t("timezone.user") }}</FieldLabel>
          <Input
            id="timezone-user"
            v-model="userId"
            type="number"
            min="0"
            max="21473"
            step="1"
            required
            dir="ltr"
          />
          <FieldDescription>{{ t("timezone.userHelp") }}</FieldDescription>
        </Field>
        <Field>
          <FieldLabel for="timezone-id">{{ t("timezone.zone") }}</FieldLabel>
          <Select v-model="timezone">
            <SelectTrigger class="w-full" dir="ltr" :aria-label="t('timezone.presets')"
              ><SelectValue
            /></SelectTrigger>
            <SelectContent
              ><SelectItem v-for="zone in presets" :key="zone" :value="zone" dir="ltr">{{
                zone
              }}</SelectItem></SelectContent
            >
          </Select>
          <Input
            id="timezone-id"
            v-model="timezone"
            required
            maxlength="64"
            pattern="[uU][tT][cC]|[a-zA-Z][a-zA-Z0-9_+-]{0,31}(/[a-zA-Z][a-zA-Z0-9_+-]{0,31}){1,2}"
            dir="ltr"
            autocapitalize="off"
            spellcheck="false"
            placeholder="America/New_York"
          />
          <FieldDescription>{{ t("timezone.zoneHelp") }}</FieldDescription>
        </Field>
        <DialogFooter>
          <Button type="button" variant="outline" @click="open = false">{{
            coreT("core.actions.cancel")
          }}</Button>
          <Button type="submit">{{ coreT("core.actions.done") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
