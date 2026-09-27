import { useEffect, useState } from "preact/hooks";

import { categoryDefinitions, type Category } from "../catalog.ts";

export interface HashRoute {
  readonly category?: Category;
  readonly slug?: string;
}

export interface DefaultHashRoute {
  readonly category: Category;
  readonly slug: string;
}

const knownCategories = new Set<Category>(categoryDefinitions.map(category => category.value));

function isCategory(value: string | undefined): value is Category {
  return value !== undefined && knownCategories.has(value as Category);
}

function decodePart(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

/** Parses `#/category/slug` and the shorter `#/components/slug` form. */
export function parseHashRoute(hash: string): HashRoute {
  const parts = hash.replace(/^#/, "").split("/").filter(Boolean).map(decodePart);
  if (parts[0] === "components") {
    return { slug: parts[1] };
  }

  if (isCategory(parts[0])) {
    return { category: parts[0], slug: parts[1] };
  }

  return { slug: parts[0] };
}

export function formatHashRoute(category: Category, slug?: string): string {
  return slug ? `#/${category}/${slug}` : `#/${category}`;
}

function readRoute(fallback: DefaultHashRoute): HashRoute {
  if (typeof window === "undefined" || !window.location.hash) {
    return fallback;
  }
  return parseHashRoute(window.location.hash);
}

/** Keeps the selected component synchronized with the browser hash and history. */
export function useHashRoute(fallback: DefaultHashRoute) {
  const [route, setRoute] = useState<HashRoute>(() => readRoute(fallback));

  useEffect(() => {
    if (!window.location.hash) {
      window.history.replaceState(null, "", formatHashRoute(fallback.category, fallback.slug));
    }

    const onHashChange = () => setRoute(parseHashRoute(window.location.hash));
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, [fallback.category, fallback.slug]);

  function navigate(category: Category, slug?: string) {
    const nextHash = formatHashRoute(category, slug);
    setRoute({ category, slug });
    if (typeof window !== "undefined" && window.location.hash !== nextHash) {
      window.location.hash = nextHash;
    }
  }

  return { route, navigate };
}
