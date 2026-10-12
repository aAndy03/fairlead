export interface Product {
  id: string;
  name: string;
  priceCents: number;
  stock: number;
}

export const CATALOG: Product[] = [
  { id: "mug", name: "Enamel mug", priceCents: 1450, stock: 12 },
  { id: "tote", name: "Canvas tote", priceCents: 2200, stock: 0 },
  { id: "notebook", name: "Dot notebook", priceCents: 900, stock: 3 },
];

export function findProduct(id: string, catalog: Product[] = CATALOG): Product | undefined {
  return catalog.find((p) => p.id === id);
}

export function inStock(product: Product): boolean {
  return product.stock > 0;
}
