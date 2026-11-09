// Tema seçimi yalnızca bu bilgisayarda (tarayıcı depolamasında) tutulur.
export type Theme = "system" | "dark" | "light";
const KEY = "bosun.theme";

export function cachedTheme(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === "dark" || v === "light" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(t: Theme) {
  if (t === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", t);
  try {
    localStorage.setItem(KEY, t);
  } catch {
    /* depolama yoksa önemli değil */
  }
}
