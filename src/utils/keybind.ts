const MODIFIER_LABELS: Record<string, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
  shift: "Shift",
};

const KEY_LABELS: Record<string, string> = {
  UP: "Arrow Up",
  DOWN: "Arrow Down",
  LEFT: "Arrow Left",
  RIGHT: "Arrow Right",
  SPACE: "Space",
  RETURN: "Enter",
  ENTER: "Enter",
  ESCAPE: "Escape",
  ESC: "Escape",
  TAB: "Tab",
  BACK: "Backspace",
  BACKSPACE: "Backspace",
  DELETE: "Delete",
  DEL: "Delete",
  INSERT: "Insert",
  HOME: "Home",
  END: "End",
  PRIOR: "Page Up",
  NEXT: "Page Down",
  PAGEUP: "Page Up",
  PAGEDOWN: "Page Down",
  CAPITAL: "Caps Lock",
  NUMLOCK: "Num Lock",
  SCROLL: "Scroll Lock",
  LWIN: "Windows",
  RWIN: "Windows",
};

for (let i = 1; i <= 24; i++) KEY_LABELS[`F${i}`] = `F${i}`;
for (let i = 0; i <= 9; i++) KEY_LABELS[`NUMPAD${i}`] = `Numpad ${i}`;

function titleCaseFallback(token: string): string {
  return token
    .toLowerCase()
    .split(/[_\s]+/)
    .filter(Boolean)
    .map((word) => word[0].toUpperCase() + word.slice(1))
    .join(" ");
}

/** 3DMigoto ini key syntax ("ctrl VK_UP", "no_modifiers UP", "h") -> a human label
 * ("Ctrl + Arrow Up", "Arrow Up", "H"), for display only — never sent back to the ini. */
export function formatKeybind(raw: string): string {
  const tokens = raw.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return raw;

  const keyToken = tokens[tokens.length - 1];
  if (keyToken.toLowerCase() === "no_modifiers") return raw;

  const parts: string[] = [];
  for (const token of tokens.slice(0, -1)) {
    const label = MODIFIER_LABELS[token.toLowerCase()];
    if (label) parts.push(label);
  }

  const withoutVk = keyToken.replace(/^VK_/i, "");
  const upper = withoutVk.toUpperCase();
  const keyLabel =
    KEY_LABELS[upper] ?? (withoutVk.length === 1 ? withoutVk.toUpperCase() : titleCaseFallback(withoutVk));

  parts.push(keyLabel);
  return parts.join(" + ");
}
