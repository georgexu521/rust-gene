import { useState, useEffect } from "react";

export type ThemeMode = "dark" | "light";

const THEME_KEY = "priority-agent.theme";

function getSystemTheme(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function storedTheme(): ThemeMode | null {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "dark" || stored === "light") return stored;
  } catch {
    /* localStorage unavailable */
  }
  return null;
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(() => storedTheme() ?? getSystemTheme());

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
  }, [theme]);

  // Listen for system theme changes when no stored preference
  useEffect(() => {
    if (storedTheme()) return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const handler = (e: MediaQueryListEvent) => setThemeState(e.matches ? "light" : "dark");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  function toggle() {
    setThemeState((t) => (t === "dark" ? "light" : "dark"));
  }

  return { theme, toggle };
}
