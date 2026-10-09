"use client";
// fairlead: def cmp.sync-button sym=SyncButton layer=molecule
// fairlead: use tok.primary cmp.button

export function SyncButton() {
  return (
    <Button>
      {/* fairlead: use tok.surface */}
      <Icon />
    </Button>
  );
}
