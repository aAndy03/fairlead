+++
title = "Design tokens"
directives = [
  'def tok.color.primary value="#2563eb" applies=color,background,background-color,border-color sum="brand colour"',
  'def tok.color.on-primary value="#ffffff" applies=color sum="text on the brand colour"',
  'def tok.color.surface value="#ffffff" applies=background,background-color sum="panel background"',
  'def tok.color.text value="#111827" applies=color sum="body text"',
  'def tok.color.muted value="#6b7280" applies=color sum="secondary text"',
  'def tok.color.border value="#e5e7eb" applies=border-color sum="hairlines"',
  'def tok.color.success value="#16a34a" applies=color,border-color sum="positive state"',
  'def tok.color.danger value="#dc2626" applies=color,border-color sum="negative state"',
  'def tok.space.1 value="4px" applies=padding,margin,gap sum="smallest gap"',
  'def tok.space.2 value="8px" applies=padding,margin,gap sum="small gap"',
  'def tok.space.4 value="16px" applies=padding,margin,gap sum="base gap"',
  'def tok.radius value="6px" applies=border-radius sum="corner radius"',
]
+++

The kit's tokens. `kit/tokens.css` holds the values and `kit/tokens.ts` exposes them to code as
`var(--...)`; apps use `tokens` from the kit, never a raw value.
