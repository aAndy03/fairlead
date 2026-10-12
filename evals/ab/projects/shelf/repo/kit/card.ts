// fairlead: def cmp.card layer=ui.molecule container=ctr.kit vis=public sum="a titled panel"
import { escapeHtml } from "./escape.ts";
import { tokens } from "./tokens.ts";

export interface CardProps {
  title: string;
  body: string; // already-rendered HTML
  footer?: string; // already-rendered HTML
}

export function renderCard(props: CardProps): string {
  const style = `background:${tokens.color.surface};border:1px solid ${tokens.color.border};border-radius:${tokens.radius};padding:${tokens.space[4]}`;
  const footer = props.footer ? `<footer>${props.footer}</footer>` : "";
  return `<section class="card" style="${style}"><h2>${escapeHtml(props.title)}</h2>${props.body}${footer}</section>`;
}
