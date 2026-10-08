import type { ScheduleTrigger } from "../types/settings";

export const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function pad(value: number): string {
  return value.toString().padStart(2, "0");
}

export function describeTrigger(trigger: ScheduleTrigger): string {
  switch (trigger.kind) {
    case "once":
      return `Once · ${new Date(trigger.at_unix_ms).toLocaleString()}`;
    case "daily":
      return `Daily · ${pad(trigger.hour)}:${pad(trigger.minute)}`;
    case "weekly": {
      const days = WEEKDAY_LABELS.filter((_, index) => (trigger.weekdays & (1 << index)) !== 0);
      return `Weekly · ${days.join(", ") || "no days"} · ${pad(trigger.hour)}:${pad(trigger.minute)}`;
    }
    case "file_appears":
      return `When '${trigger.pattern}' appears in ${trigger.folder}`;
  }
}
