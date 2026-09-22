export function isTheme(value) {
  return value === "light" || value === "dark";
}

export function savedTheme(host) {
  try {
    const value = host.localStorage.getItem("progred-theme");
    return isTheme(value) ? value : "light";
  } catch {
    return "light";
  }
}

export function saveTheme(host, theme) {
  try { host.localStorage.setItem("progred-theme", theme); } catch { /* Optional preference. */ }
}
