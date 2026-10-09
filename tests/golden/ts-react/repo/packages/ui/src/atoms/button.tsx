// fairlead: def cmp.button
// fairlead: use tok.color.primary
export function Button(props: { label: string }) {
  return <button className="rounded-md">{props.label}</button>;
}
