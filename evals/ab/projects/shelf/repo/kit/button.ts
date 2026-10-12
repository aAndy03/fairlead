// fairlead: def ctr.kit path="kit/**"
// fairlead: def cmp.button layer=ui.atom container=ctr.kit vis=public sum="the shop's button"
import { escapeHtml } from "./escape.ts";
import { tokens } from "./tokens.ts";

export type ButtonVariant = "primary" | "secondary";

export interface ButtonProps {
  label: string;
  variant?: ButtonVariant;
  disabled?: boolean;
  action?: string;
}

export function renderButton(props: ButtonProps): string {
  const variant = props.variant ?? "primary";
  const background = variant === "primary" ? tokens.color.primary : tokens.color.surface;
  const color = variant === "primary" ? tokens.color.onPrimary : tokens.color.text;
  const border = variant === "primary" ? "none" : `1px solid ${tokens.color.border}`;
  const style = [
    `background:${background}`,
    `color:${color}`,
    `border:${border}`,
    `padding:${tokens.space[2]} ${tokens.space[4]}`,
    `border-radius:${tokens.radius}`,
  ].join(";");
  const action = props.action ? ` data-action="${escapeHtml(props.action)}"` : "";
  const disabled = props.disabled ? " disabled" : "";
  return `<button class="btn btn-${variant}" style="${style}"${action}${disabled}>${escapeHtml(props.label)}</button>`;
}
