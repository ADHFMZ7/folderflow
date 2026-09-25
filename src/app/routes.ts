// Pages of the main window, kept in the URL hash so a reload stays put.

import { useEffect, useState } from "react";

export const PAGES = ["workflows", "history", "templates", "settings"] as const;
export type Page = (typeof PAGES)[number];
export type Route = { page: Page };

export function parseHash(hash: string): Route {
  const page = hash.replace(/^#\/?/, "").split("/")[0];
  return { page: (PAGES as readonly string[]).includes(page) ? (page as Page) : "workflows" };
}

export const hrefFor = (route: Route) => `#/${route.page}`;

export function useRoute(): Route {
  const [route, setRoute] = useState(() => parseHash(window.location.hash));
  useEffect(() => {
    const onChange = () => setRoute(parseHash(window.location.hash));
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);
  return route;
}
