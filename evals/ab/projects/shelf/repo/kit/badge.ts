// fairlead: def cmp.badge layer=ui.atom container=ctr.kit vis=public sum="a small status label"
import { escapeHtml } from "./escape.ts";
import { tokens } from "./tokens.ts";

export type BadgeTone = "info" | "success" | "danger";

const TONE: Record<BadgeTone, string> = {
  info: tokens.color.primary,
  success: tokens.color.success,
  danger: tokens.color.danger,
};

export function renderBadge(text: string, tone: BadgeTone = "info"): string {
  const style = `color:${TONE[tone]};border:1px solid ${TONE[tone]};padding:0 ${tokens.space[1]};border-radius:${tokens.radius}`;
  return `<span class="badge badge-${tone}" style="${style}">${escapeHtml(text)}</span>`;
}
